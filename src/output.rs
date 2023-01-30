use cpal::traits::{DeviceTrait, StreamTrait};
use rand::prelude::*;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::grain::{GrainEngine, Grain, GrainSettings};


pub struct OutputStream {
	_stream: cpal::Stream,

	state: Arc<Mutex<OutputStreamState>>,
}


#[derive(Default)]
pub struct OutputStreamState {
	pub play_buffer: Vec<f32>,
	pub cursor: usize,

	pub range: Option<(usize, usize)>,

	pub grains: GrainEngine,
}


impl OutputStream {
	pub fn start(device: &cpal::Device, config: cpal::StreamConfig) -> anyhow::Result<OutputStream> {
		let state = Arc::new(Mutex::new(OutputStreamState::default()));
		let callback = Callback { state: state.clone() };

		let stream = device.build_output_stream(
			&config,
			
			move |data: &mut [f32], callback_info: &cpal::OutputCallbackInfo| {
				callback.process(data, callback_info);
			},

			move |_err| {
				println!("OutputStream Error {_err}");
			}
		)?;

		stream.play()?;

		Ok(OutputStream {
			_stream: stream,
			state,
		})
	}

	pub async fn set_play_buffer(&self, buffer: Vec<f32>) {
		let mut state = self.state.lock().await;
		state.play_buffer = buffer;
		state.range = None;
		state.cursor = 0;
		state.grains.clear();
	}

	pub async fn clear_play_buffer(&self) {
		let mut state = self.state.lock().await;
		state.play_buffer = Vec::new();
	}

	pub async fn play_cursor(&self) -> usize {
		self.state.lock().await.cursor
	}

	pub async fn set_play_range(&self, start: Option<usize>, end: Option<usize>) {
		let mut state = self.state.lock().await;
		let num_samples = state.play_buffer.len();
		state.range = Some((start.unwrap_or(0), end.unwrap_or(num_samples)));
	}

	pub async fn set_grain_settings(&self, settings: GrainSettings) {
		let mut state = self.state.lock().await;
		state.grains.settings = settings;
	}
}



struct Callback {
	state: Arc<Mutex<OutputStreamState>>,
}

impl Callback {
	fn process(&self, data: &mut [f32], _: &cpal::OutputCallbackInfo) {
		let mut state = self.state.blocking_lock();

		let OutputStreamState {play_buffer, cursor, range, grains} = &mut *state;

		data.fill(0.0);

		let (min, max) = range.unwrap_or((0, play_buffer.len()));
		if min >= max || max > play_buffer.len() {
			return;
		}

		let width = max - min;

		while *cursor < min {
			*cursor += width;
		}

		*cursor = (*cursor - min + data.len() / 2) % width + min;


		grains.update();

		let cursor_spawn_jitter = grains.settings.cursor_spawn_jitter;
		let stereo_width = grains.settings.stereo_width;

		while grains.grains.len() < grains.settings.num_grains {
			let mut rng = rand::thread_rng();
			let mut grain_start = rng.gen_range::<usize, _>(cursor.saturating_sub(cursor_spawn_jitter)..=(*cursor + cursor_spawn_jitter).min(play_buffer.len()));

			while grain_start < min {
				grain_start += width;
			}
			while grain_start > max {
				grain_start -= width;
			}

			let grain_len = rng.gen_range::<usize, _>(grains.settings.grain_length_min..=grains.settings.grain_length_max);
			let pan = rng.gen_range(-stereo_width..=stereo_width);

			grains.grains.push(Grain::new(grain_start, (grain_start + grain_len).min(play_buffer.len()), pan));
		}

		for grain in grains.grains.iter_mut() {
			grain.process_into(data, &play_buffer);
		}
	}
}





