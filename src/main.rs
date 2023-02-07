#![feature(array_chunks)]

use anyhow::Result;


mod input;
mod output;
mod coordinator;
mod grain;
mod view;

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


			let display_state = self.coordinator.display_state();
			let waveform_widget = view::Waveform {
				buffer: &display_state.play_buffer,
				selection_start: &mut self.settings.range_start,
				selection_end: &mut self.settings.range_end,
				cursor: display_state.cursor,
			};

			ui.add(waveform_widget);
		});

		ctx.request_repaint();

		self.coordinator.set_settings(self.settings.clone());
	}
}




