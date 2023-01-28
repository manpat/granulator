#![feature(array_chunks)]

use eframe::egui;
use anyhow::Result;

use cpal::traits::{HostTrait, DeviceTrait};
// use rand::prelude::*;

// use tokio::task;

mod input;
mod output;


#[tokio::main]
async fn main() -> Result<()> {
	std::env::set_var("RUST_BACKTRACE", "1");

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


	let input_stream = input::InputStream::start(&input_device, input_config)?;
	let output_stream = output::OutputStream::start(&output_device, output_config)?;


	eframe::run_native("Granulator", <_>::default(), Box::new(move |_cc| {
		Box::new(AppRoot {
			input_stream,
			output_stream,

			display_buf: Vec::new(),
			range_start: None,
			range_end: None,
		})
	}));

	Ok(())
}



struct AppRoot {
	input_stream: input::InputStream,
	output_stream: output::OutputStream,

	display_buf: Vec<f32>,
	range_start: Option<usize>,
	range_end: Option<usize>,
}

impl eframe::App for AppRoot {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		egui::CentralPanel::default().show(ctx, |ui| {
			let recording = self.input_stream.lock_state().record_buffer.is_some();

			ui.horizontal(|ui| {
				if recording {
					if ui.button("Stop").clicked() {
						let buffer = self.input_stream.lock_state().record_buffer.take().unwrap();
						// self.display_buf = buffer.clone();
						self.range_start = None;
						self.range_end = None;

						// let output_state = self.output_state.clone();

						// task::spawn_blocking(move || {
							use rubato::{Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction};
							let params = InterpolationParameters {
								sinc_len: 256,
								f_cutoff: 0.95,
								interpolation: InterpolationType::Linear,
								oversampling_factor: 256,
								window: WindowFunction::BlackmanHarris2,
							};

							let mut resampler = SincFixedIn::<f32>::new(
								44100 as f64 / 48000 as f64,
								2.0,
								params,
								buffer.len(),
								1,
							).unwrap();

							let waves_in = vec![buffer];
							let mut waves_out = resampler.process(&waves_in, None).unwrap();
							let waves_out = waves_out.remove(0);

							self.display_buf = waves_out.clone();

							let mut out_state = self.output_stream.lock_state();
							out_state.play_buffer = waves_out;
							out_state.range = None;
							out_state.cursor = 0;
							out_state.grains.clear();
						// });
					}
				} else {
					if ui.button("Record").clicked() {
						let buf = Vec::with_capacity(48000);
						let mut state = self.input_stream.lock_state();
						state.record_buffer = Some(buf);
					}
				}

				let mut state = self.output_stream.lock_state();

				if ui.button("Clear").clicked() {
					self.display_buf.clear();
					state.play_buffer = Vec::new();
				}

				ui.add(egui::DragValue::new(&mut state.grains.num_grains).clamp_range(1..=200).suffix(" grains"));
				ui.add(egui::DragValue::new(&mut state.grains.grain_length_min).clamp_range(10..=10000).speed(10.0).prefix("min "));
				ui.add(egui::DragValue::new(&mut state.grains.grain_length_max).clamp_range(10..=10000).speed(10.0).prefix("max "));
				ui.add(egui::DragValue::new(&mut state.grains.cursor_spawn_jitter).clamp_range(0..=50000).speed(50.0).prefix("jitter "));
				ui.add(egui::DragValue::new(&mut state.grains.stereo_width).clamp_range(0.0..=1.0).speed(0.005).prefix("width "));

				let (min, max) = (state.grains.grain_length_min, state.grains.grain_length_max);
				state.grains.grain_length_min = min.min(max);
				state.grains.grain_length_max = min.max(max);
			});

			let (response, mut painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
			let rect = response.rect.shrink(5.0);
			painter.set_clip_rect(rect);

			let bg_color = ui.visuals().panel_fill;
			let outline_stroke = ui.visuals().window_stroke;
			let wave_stroke = egui::Stroke::new(1.0, egui::Color32::YELLOW);
			let cursor_stroke = egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE);
			let selection_color = egui::Color32::LIGHT_BLUE.linear_multiply(0.2);

			painter.rect(rect, 0.0, bg_color, outline_stroke);

			// Set selection
			{
				let mut update_output_state = false;

				if response.drag_started() {
					let num_samples = self.display_buf.len() as f32;
					let pos = response.interact_pointer_pos().unwrap();
					let sample_index = ((pos.x - rect.min.x) / rect.width() * num_samples).clamp(0.0, num_samples) as usize;

					self.range_start = Some(sample_index.min(self.display_buf.len()));
					self.range_end = self.range_start;

					update_output_state = true;
				}

				if response.dragged() {
					let num_samples = self.display_buf.len() as f32;
					let pos = response.interact_pointer_pos().unwrap();
					let sample_index = ((pos.x - rect.min.x) / rect.width() * num_samples).clamp(0.0, num_samples) as usize;

					if let Some(start) = self.range_start.as_mut() {
						if *start > sample_index {
							*start = sample_index;
						}

						update_output_state = true;
					}

					if let Some(end) = self.range_end.as_mut() {
						if *end < sample_index {
							*end = sample_index;
						}

						update_output_state = true;
					}
				}


				if update_output_state {
					let mut out_state = self.output_stream.lock_state();
					let (range_start, range_end) = (self.range_start.unwrap_or(0), self.range_end.unwrap_or(self.display_buf.len()));
					out_state.range = Some((range_start, range_end));
				}
			}

			// Draw selection
			{
				let num_samples = self.display_buf.len() as f32;
				let (range_start, range_end) = (self.range_start.unwrap_or(0), self.range_end.unwrap_or(self.display_buf.len()));

				let range_start_x = rect.min.x + range_start as f32 / num_samples * rect.width();
				let range_end_x = rect.min.x + range_end as f32 / num_samples * rect.width();

				let rect = egui::Rect::from_x_y_ranges(range_start_x..=range_end_x, rect.y_range());

				painter.rect(rect, 0.0, selection_color, egui::Stroke::NONE);
			}

			// Draw mouse cursor
			if let Some(pos) = response.hover_pos() {
				painter.vline(pos.x, rect.y_range(), cursor_stroke);
			}

			if !self.display_buf.is_empty() {
				let num_samples = self.display_buf.len();
				let display_width = rect.width();
				let display_height = rect.height();
				let center_y = rect.min.y + display_height / 2.0;

				// Draw waveform
				for x in 0..display_width as usize {
					let sample_index = (x as f32 / display_width * num_samples as f32) as usize;
					let sample = self.display_buf[sample_index].abs();

					let top = center_y - display_height * sample / 2.0;
					let bottom = center_y + display_height * sample / 2.0;

					painter.vline(rect.min.x + x as f32, top..=bottom, wave_stroke);
				}

				// Draw play cursor
				let cursor = self.output_stream.lock_state().cursor;
				let cursor_x = cursor as f32 / num_samples as f32 * display_width;

				painter.vline(rect.min.x + cursor_x, rect.y_range(), cursor_stroke);
			}
		});

		ctx.request_repaint();
	}
}





pub struct GrainEngine {
	grains: Vec<Grain>,

	num_grains: usize,
	grain_length_min: usize,
	grain_length_max: usize,
	cursor_spawn_jitter: usize,

	stereo_width: f32,
}

impl GrainEngine {
	fn clear(&mut self) {
		self.grains.clear();
	}

	fn update(&mut self) {
		self.grains.retain(|grain| {
			grain.position < grain.end
		})
	}
}

impl Default for GrainEngine {
	fn default() -> Self {
		GrainEngine {
			grains: Vec::new(),

			num_grains: 1,
			grain_length_min: 1000,
			grain_length_max: 3000,
			cursor_spawn_jitter: 0,

			stereo_width: 0.0,
		}
	}
}

struct Grain {
	position: usize,

	start: usize,
	end: usize,

	pan: f32,
}

impl Grain {
	fn new(start: usize, end: usize, pan: f32) -> Grain {
		Grain {
			start,
			end,
			pan,

			position: start,
		}
	}

	fn process_into(&mut self, output: &mut [f32], source: &[f32]) {
		if self.position < self.start || self.position >= self.end {
			return;
		}

		let base_gain = 0.3;
		let pan_norm = self.pan * 0.5 + 0.5;
		let left_gain = base_gain * (1.0 - pan_norm).max(0.0);
		let right_gain = base_gain * pan_norm.max(0.0);

		let width = self.end - self.start;

		for (index, ([out_l, out_r], in_sample)) in output.array_chunks_mut().zip(&source[self.position..self.end]).enumerate() {
			let real_pos = self.position - self.start + index;

			let env = match real_pos {
				x if x < 400 => x as f32 / 400.0,
				x if x + 400 < width => 1.0,
				x => 1.0 - (x + 400 - width) as f32 / 400.0,
			};

			*out_l += in_sample * env * left_gain;
			*out_r += in_sample * env * right_gain;
		}

		self.position += output.len() / 2;
	}
}




