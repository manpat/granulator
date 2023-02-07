


pub struct Waveform<'a> {
	pub buffer: &'a [f32],
	pub cursor: usize,

	pub selection_start: &'a mut Option<usize>,
	pub selection_end: &'a mut Option<usize>,

}


impl egui::Widget for Waveform<'_> {
	fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
		let (mut response, mut painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
		let rect = response.rect.shrink(5.0);

		response.rect = rect;
		painter.set_clip_rect(response.rect);

		let mapping = SampleDisplayMapping::new(rect.x_range(), self.buffer.len());

		let bg_color = ui.visuals().panel_fill;
		let outline_stroke = ui.visuals().window_stroke;
		let wave_stroke = egui::Stroke::new(1.0, egui::Color32::YELLOW);
		let cursor_stroke = egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE);
		let selection_color = egui::Color32::LIGHT_BLUE.linear_multiply(0.2);

		// Background
		painter.rect(rect, 0.0, bg_color, outline_stroke);

		// Handle interactions
		self.handle_input(&mut response, &mapping);

		// Draw selection
		{
			let num_samples = self.buffer.len();
			let (range_start, range_end) = (self.selection_start.unwrap_or(0), self.selection_end.unwrap_or(num_samples));

			let range_start_x = mapping.sample_to_display(range_start);
			let range_end_x = mapping.sample_to_display(range_end);

			let rect = egui::Rect::from_x_y_ranges(range_start_x..=range_end_x, rect.y_range());

			painter.rect(rect, 0.0, selection_color, egui::Stroke::NONE);
		}

		// Draw mouse cursor
		if let Some(pos) = response.hover_pos() {
			painter.vline(pos.x, rect.y_range(), cursor_stroke);
		}

		if !self.buffer.is_empty() {
			let display_height = rect.height();
			let center_y = rect.min.y + display_height / 2.0;

			// Draw waveform
			for x in 0..rect.width() as usize {
				let display_x = rect.min.x + x as f32;

				let sample_index = mapping.display_to_sample(display_x);
				let sample = self.buffer[sample_index].abs();

				let top = center_y - display_height * sample / 2.0;
				let bottom = center_y + display_height * sample / 2.0;

				painter.vline(display_x, top..=bottom, wave_stroke);
			}

			// Draw play cursor
			let cursor_x = mapping.sample_to_display(self.cursor);
			painter.vline(cursor_x, rect.y_range(), cursor_stroke);
		}

		response
	}
}

impl Waveform<'_> {
	fn handle_input(&mut self, response: &mut egui::Response, mapping: &SampleDisplayMapping) {
		if response.drag_started() {
			let pos = response.interact_pointer_pos().unwrap();
			let sample_index = mapping.display_to_sample(pos.x);

			*self.selection_start = Some(sample_index);
			*self.selection_end = *self.selection_start;

			response.mark_changed();
		}

		if response.dragged() {
			let pos = response.interact_pointer_pos().unwrap();
			let sample_index = mapping.display_to_sample(pos.x);

			if let Some(start) = self.selection_start {
				if *start > sample_index {
					*start = sample_index;
					response.mark_changed();
				}
			}

			if let Some(end) = self.selection_end {
				if *end < sample_index {
					*end = sample_index;
					response.mark_changed();
				}
			}
		}
	}
}


use std::ops::RangeInclusive;


struct SampleDisplayMapping {
	display_start: f32,
	display_to_sample_ratio: f32,
	num_samples: usize,
}

impl SampleDisplayMapping {
	pub fn new(display_range: RangeInclusive<f32>, num_samples: usize) -> Self {
		let (display_start, display_end) = display_range.into_inner();
		let display_width = display_end - display_start;
		let display_to_sample_ratio = num_samples as f32 / display_width;

		SampleDisplayMapping {
			display_start,
			display_to_sample_ratio,
			num_samples,
		}
	}

	pub fn sample_to_display(&self, sample: usize) -> f32 {
		self.sample_to_display_magnitude(sample) + self.display_start
	}

	pub fn sample_to_display_magnitude(&self, sample: usize) -> f32 {
		sample as f32 / self.display_to_sample_ratio
	}

	pub fn display_to_sample(&self, display: f32) -> usize {
		(((display - self.display_start) * self.display_to_sample_ratio) as usize)
			.min(self.num_samples)
	}
}