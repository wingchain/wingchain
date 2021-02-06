// Copyright 2019, 2020 Wingchain
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use futures::prelude::*;
use futures::task::{Context, Poll};
use futures_timer::Delay;
use std::pin::Pin;
use std::time::{Duration, SystemTime};

pub struct Scheduler {
	duration: Option<u64>,
	delay: Option<Delay>,
}

impl Scheduler {
	pub fn new(duration: Option<u64>) -> Self {
		Self {
			duration,
			delay: None,
		}
	}
}

pub struct ScheduleInfo {
	pub timestamp: u64,
}

impl Stream for Scheduler {
	type Item = ScheduleInfo;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let duration = match self.duration {
			Some(v) => v,
			None => return Poll::Pending,
		};

		self.delay = match self.delay.take() {
			None => {
				// schedule wait.
				let wait_duration = time_until_next(duration_now(), duration);
				Some(Delay::new(wait_duration))
			}
			Some(d) => Some(d),
		};

		if let Some(ref mut delay) = self.delay {
			match Future::poll(Pin::new(delay), cx) {
				Poll::Pending => return Poll::Pending,
				Poll::Ready(()) => {}
			}
		}

		self.delay = None;
		let timestamp = duration_now().as_millis() as u64;
		let schedule_info = ScheduleInfo { timestamp };

		Poll::Ready(Some(schedule_info))
	}
}

fn time_until_next(now: Duration, duration: u64) -> Duration {
	let remaining_full_millis = duration - (now.as_millis() as u64 % duration) - 1;
	Duration::from_millis(remaining_full_millis)
}

fn duration_now() -> Duration {
	let now = SystemTime::now();
	now.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or_else(|e| {
			panic!(
				"Current time {:?} is before unix epoch. Something is wrong: {:?}",
				now, e,
			)
		})
}
