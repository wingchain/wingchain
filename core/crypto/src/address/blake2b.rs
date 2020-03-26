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

use rust_crypto::blake2b;

use crate::address::Address;
use crate::AddressLength;

pub struct Blake2b160;

impl Address for Blake2b160 {
	fn name(&self) -> String {
		"blake2b160".to_string()
	}
	fn length(&self) -> AddressLength {
		AddressLength::AddressLength20
	}
	fn address(&self, out: &mut [u8], data: &[u8]) {
		assert_eq!(out.len(), self.length().into());
		blake2b::Blake2b::blake2b(out, data, &[]);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_address_blake2b_160() {
		let data = (0u8..32).collect::<Vec<_>>();
		let mut out = [0u8; 20];
		Blake2b160.address(&mut out, &data);

		assert_eq!(
			out,
			[
				177, 177, 51, 185, 159, 81, 110, 108, 130, 206, 218, 137, 46, 245, 175, 80, 250,
				75, 78, 113
			]
		);
	}
}
