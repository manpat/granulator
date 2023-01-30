use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::Arc;
use tokio::sync::Mutex;


pub struct InputStream {
	_stream: cpal::Stream,

	state: Arc<Mutex<InputStreamState>>,
}

#[derive(Default)]
pub struct InputStreamState {
	record_buffer: Option<Vec<f32>>,
}



impl InputStream {
	pub fn start(device: &cpal::Device, config: cpal::StreamConfig) -> anyhow::Result<InputStream> {
		let state = Arc::new(Mutex::new(InputStreamState::default()));
		let callback = Callback { state: state.clone() };

		let stream = device.build_input_stream(
			&config,
			move |data: &[f32], callback_info: &cpal::InputCallbackInfo| {
				callback.process(data, callback_info)
			},

			move |_err| {
				println!("InputStream Error {_err}");
			}
		)?;

		stream.play()?;

		Ok(InputStream {
			_stream: stream,
			state,
		})
	}

	pub async fn start_record(&self) {
		let buf = Vec::with_capacity(48000);

		let mut state = self.state.lock().await;
		state.record_buffer = Some(buf);
	}

	pub async fn stop_record(&self) -> Option<Vec<f32>> {
		let mut state = self.state.lock().await;
		state.record_buffer.take()
	}
}



struct Callback {
	state: Arc<Mutex<InputStreamState>>,
}

impl Callback {
	fn process(&self, data: &[f32], _: &cpal::InputCallbackInfo) {
		let mut state = self.state.blocking_lock();

		if let Some(rec_buf) = state.record_buffer.as_mut() {
			rec_buf.extend_from_slice(&data);
		}
	}
}