
#[derive(Clone, Debug)]
pub struct GrainSettings {
	pub num_grains: usize,
	pub grain_length_min: usize,
	pub grain_length_max: usize,
	pub cursor_spawn_jitter: usize,

	pub stereo_width: f32,
}


pub struct GrainEngine {
	pub grains: Vec<Grain>,

	pub settings: GrainSettings,
}

impl GrainEngine {
	pub fn clear(&mut self) {
		self.grains.clear();
	}

	pub fn update(&mut self) {
		self.grains.retain(|grain| {
			grain.position < grain.end
		})
	}
}

impl Default for GrainEngine {
	fn default() -> Self {
		GrainEngine {
			grains: Vec::new(),

			settings: GrainSettings {
				num_grains: 1,
				grain_length_min: 1000,
				grain_length_max: 3000,
				cursor_spawn_jitter: 0,

				stereo_width: 0.0,
			}
		}
	}
}

pub struct Grain {
	pub position: usize,

	pub start: usize,
	pub end: usize,

	pub pan: f32,
}

impl Grain {
	pub fn new(start: usize, end: usize, pan: f32) -> Grain {
		Grain {
			start,
			end,
			pan,

			position: start,
		}
	}

	pub fn process_into(&mut self, output: &mut [f32], source: &[f32]) {
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




