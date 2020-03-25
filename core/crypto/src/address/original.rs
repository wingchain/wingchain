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

use crate::address::Address;
use crate::AddressLength;

pub struct Original160;

impl Address for Original160 {
	fn name(&self) -> String {
		"original_160".to_string()
	}
	fn length(&self) -> AddressLength {
		AddressLength::AddressLength20
	}
	fn address(&self, out: &mut [u8], data: &[u8]) {
		assert_eq!(out.len(), self.length().into());
		out.copy_from_slice(data);
	}
}

pub struct Original256;

impl Address for Original256 {
	fn name(&self) -> String {
		"original_256".to_string()
	}
	fn length(&self) -> AddressLength {
		AddressLength::AddressLength32
	}
	fn address(&self, out: &mut [u8], data: &[u8]) {
		assert_eq!(out.len(), self.length().into());
		out.copy_from_slice(data);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_address_original_160() {
		let data = (0u8..20).collect::<Vec<_>>();
		let mut out = [0u8; 20];
		Original160.address(&mut out, &data);

		assert_eq!(
			out.to_vec(),
			data
		);
	}

	#[test]
	fn test_address_original_256() {
		let data = (0u8..32).collect::<Vec<_>>();
		let mut out = [0u8; 32];
		Original256.address(&mut out, &data);

		assert_eq!(
			out.to_vec(),
			data
		);
	}
}
