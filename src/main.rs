#![feature(array_chunks)]

use eframe::egui;
use anyhow::Result;


mod input;
mod output;
mod coordinator;
mod grain;

use coordinator::{Coordinator, PlaySettings};


#[tokio::main]
async fn main() -> Result<()> {
	std::env::set_var("RUST_BACKTRACE", "1");

	let coordinator = Coordinator::start()?;

	let settings = PlaySettings {
		grain: grain::GrainSettings {
			num_grains: 1,
			grain_length_min: 1000,
			grain_length_max: 3000,
			cursor_spawn_jitter: 0,

			stereo_width: 0.0,
		},

		range_start: None,
		range_end: None,
	};

	coordinator.set_settings(settings.clone());

	eframe::run_native("Granulator", <_>::default(), Box::new(move |_cc| {
		Box::new(AppRoot {
			coordinator,
			settings,
		})
	}));

	Ok(())
}



struct AppRoot {
	coordinator: Coordinator,
	settings: PlaySettings,
}

impl eframe::App for AppRoot {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		egui::CentralPanel::default().show(ctx, |ui| {
			let recording = self.coordinator.display_state().is_recording;
			let num_samples = self.coordinator.display_state().play_buffer.len();

			ui.horizontal(|ui| {
				if recording {
					if ui.button("Stop").clicked() {
						self.coordinator.stop_record();
						self.settings.range_start = None;
						self.settings.range_end = None;
					}
				} else {
					if ui.button("Record").clicked() {
						self.coordinator.start_record();
					}
				}

				if ui.button("Clear").clicked() {
					self.coordinator.clear_play_buffer();
				}

				ui.add(egui::DragValue::new(&mut self.settings.grain.num_grains).clamp_range(1..=200).suffix(" grains"));
				ui.add(egui::DragValue::new(&mut self.settings.grain.grain_length_min).clamp_range(800..=10000).speed(10.0).prefix("min "));
				ui.add(egui::DragValue::new(&mut self.settings.grain.grain_length_max).clamp_range(800..=10000).speed(10.0).prefix("max "));
				ui.add(egui::DragValue::new(&mut self.settings.grain.cursor_spawn_jitter).clamp_range(0..=50000).speed(50.0).prefix("jitter "));
				ui.add(egui::DragValue::new(&mut self.settings.grain.stereo_width).clamp_range(0.0..=1.0).speed(0.005).prefix("width "));

				let (min, max) = (self.settings.grain.grain_length_min, self.settings.grain.grain_length_max);
				self.settings.grain.grain_length_min = min.min(max);
				self.settings.grain.grain_length_max = min.max(max);
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
				if response.drag_started() {
					let num_samples_f = num_samples as f32;
					let pos = response.interact_pointer_pos().unwrap();
					let sample_index = ((pos.x - rect.min.x) / rect.width() * num_samples_f).clamp(0.0, num_samples_f) as usize;

					self.settings.range_start = Some(sample_index.min(num_samples));
					self.settings.range_end = self.settings.range_start;
				}

				if response.dragged() {
					let num_samples_f = num_samples as f32;
					let pos = response.interact_pointer_pos().unwrap();
					let sample_index = ((pos.x - rect.min.x) / rect.width() * num_samples_f).clamp(0.0, num_samples_f) as usize;

					if let Some(start) = self.settings.range_start.as_mut() {
						if *start > sample_index {
							*start = sample_index;
						}
					}

					if let Some(end) = self.settings.range_end.as_mut() {
						if *end < sample_index {
							*end = sample_index;
						}
					}
				}
			}

			// Draw selection
			{
				let (range_start, range_end) = (self.settings.range_start.unwrap_or(0), self.settings.range_end.unwrap_or(num_samples));

				let num_samples = num_samples as f32;
				let range_start_x = rect.min.x + range_start as f32 / num_samples * rect.width();
				let range_end_x = rect.min.x + range_end as f32 / num_samples * rect.width();

				let rect = egui::Rect::from_x_y_ranges(range_start_x..=range_end_x, rect.y_range());

				painter.rect(rect, 0.0, selection_color, egui::Stroke::NONE);
			}

			// Draw mouse cursor
			if let Some(pos) = response.hover_pos() {
				painter.vline(pos.x, rect.y_range(), cursor_stroke);
			}

			if num_samples > 0 {
				let display_width = rect.width();
				let display_height = rect.height();
				let center_y = rect.min.y + display_height / 2.0;

				let display_state = self.coordinator.display_state();

				// Draw waveform
				for x in 0..display_width as usize {
					let sample_index = (x as f32 / display_width * num_samples as f32) as usize;
					let sample = display_state.play_buffer[sample_index].abs();

					let top = center_y - display_height * sample / 2.0;
					let bottom = center_y + display_height * sample / 2.0;

					painter.vline(rect.min.x + x as f32, top..=bottom, wave_stroke);
				}

				// Draw play cursor
				let cursor = display_state.cursor;
				let cursor_x = cursor as f32 / num_samples as f32 * display_width;

				painter.vline(rect.min.x + cursor_x, rect.y_range(), cursor_stroke);
			}
		});

		ctx.request_repaint();

		self.coordinator.set_settings(self.settings.clone());
	}
}




