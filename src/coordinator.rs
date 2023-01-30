use tokio::sync::mpsc;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task;
use tokio::runtime::Handle;
use std::sync::Arc;

use cpal::traits::{HostTrait, DeviceTrait};

use crate::{input, output, grain::GrainSettings};



pub struct Coordinator {
	cmd_tx: mpsc::Sender<Command>,

	display_state: Arc<Mutex<DisplayState>>,
}

#[derive(Default)]
pub struct DisplayState {
	pub play_buffer: Vec<f32>,
	pub cursor: usize,
	pub is_recording: bool,
}


#[derive(Clone, Debug)]
pub struct PlaySettings {
	pub grain: GrainSettings,

	pub range_start: Option<usize>,
	pub range_end: Option<usize>,
}



impl Coordinator {
	pub fn start() -> anyhow::Result<Coordinator> {
		let (cmd_tx, cmd_rx) = mpsc::channel(16);
		let display_state = Arc::new(Mutex::new(DisplayState::default()));
		let async_handle = Handle::current();

		std::thread::spawn({
			let display_state = display_state.clone();

			move || {
				start_inner(cmd_rx, display_state, async_handle)
			}
		});

		Ok(Coordinator {
			cmd_tx,
			display_state,
		})
	}

	pub fn start_record(&self) {
		self.cmd_tx.try_send(Command::StartRecord)
			.unwrap();
	}

	pub fn stop_record(&self) {
		self.cmd_tx.try_send(Command::StopRecord)
			.unwrap();
	}

	pub fn clear_play_buffer(&self) {
		self.cmd_tx.try_send(Command::ClearPlayBuffer)
			.unwrap();
	}

	pub fn set_settings(&self, settings: PlaySettings) {
		self.cmd_tx.try_send(Command::SetSettings(settings))
			.unwrap();
	}

	pub fn display_state(&self) -> MutexGuard<'_, DisplayState> {
		task::block_in_place(|| self.display_state.blocking_lock())
	}
}



#[derive(Debug)]
enum Command {
	StartRecord,
	StopRecord,

	ClearPlayBuffer,

	SetSettings(PlaySettings),
}



fn start_inner(mut cmd_rx: mpsc::Receiver<Command>, display_state: Arc<Mutex<DisplayState>>,
	async_handle: tokio::runtime::Handle) -> anyhow::Result<()>
{
	let host = cpal::default_host();
	let output_device = host.default_output_device().ok_or_else(|| anyhow::Error::msg("No default output device"))?;
	let input_device = host.default_input_device().ok_or_else(|| anyhow::Error::msg("No default input device"))?;


	println!("{:?}", input_device.default_input_config());
	println!("{:?}", output_device.default_output_config());

	let input_config = cpal::StreamConfig {
		channels: 1,
		.. input_device.default_input_config()?.config()
	};

	let output_config = cpal::StreamConfig {
		channels: 2,
		.. output_device.default_output_config()?.config()
	};


	let resample_ratio = output_config.sample_rate.0 as f64 / input_config.sample_rate.0 as f64;

	let input_stream = input::InputStream::start(&input_device, input_config)?;
	let output_stream = output::OutputStream::start(&output_device, output_config)?;

	// Required because cpal::Stream is not Send and this infects InputStream and OutputStream.
	async_handle.block_on(async move {
		use tokio::time::MissedTickBehavior;

		let mut interval = tokio::time::interval(std::time::Duration::from_millis(16));
		interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

		loop {
			tokio::select!{
				value = cmd_rx.recv() => match value {
					Some(cmd) => match cmd {
						Command::StartRecord => {
							input_stream.start_record().await;
							display_state.lock().await.is_recording = true;
						}

						Command::StopRecord => {
							if let Some(buffer) = input_stream.stop_record().await {
								let buffer = task::spawn_blocking(move || resample(buffer, resample_ratio)).await?;
								output_stream.set_play_buffer(buffer.clone()).await;

								let mut display_state = display_state.lock().await;
								display_state.is_recording = false;
								display_state.play_buffer = buffer;
							}
						}

						Command::ClearPlayBuffer => {
							output_stream.clear_play_buffer().await;
						}

						Command::SetSettings(settings) => {
							output_stream.set_play_range(settings.range_start, settings.range_end).await;
							output_stream.set_grain_settings(settings.grain).await;
						}
					}

					None => break,
				},

				_ = interval.tick() => {
					let cursor = output_stream.play_cursor().await;

					let mut display_state = display_state.lock().await;
					display_state.cursor = cursor;
				}
			}
		}

		Ok(())
	})
}


fn resample(buffer: Vec<f32>, sample_rate_ratio: f64) -> Vec<f32> {
	use rubato::{Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction};
	let params = InterpolationParameters {
		sinc_len: 256,
		f_cutoff: 0.95,
		interpolation: InterpolationType::Linear,
		oversampling_factor: 256,
		window: WindowFunction::BlackmanHarris2,
	};

	let mut resampler = SincFixedIn::<f32>::new(
		sample_rate_ratio,
		2.0,
		params,
		buffer.len(),
		1,
	).unwrap();

	let waves_in = vec![buffer];
	let mut waves_out = resampler.process(&waves_in, None).unwrap();
	waves_out.remove(0)
}