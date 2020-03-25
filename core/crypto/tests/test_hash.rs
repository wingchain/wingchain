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

use crypto::hash::Hash;
use crypto::hash::HashImpl;
use std::str::FromStr;

#[test]
fn test() {
	let hash = HashImpl::Blake2b256;
	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 32];
	hash.hash(&mut out, &data);
	assert_eq!(
		out,
		[
			17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142, 220,
			157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
		]
	);
}

#[test]
fn test_from_str_blake2b_256() {
	let hash = HashImpl::from_str("blake2b_256").unwrap();
	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 32];
	hash.hash(&mut out, &data);
	assert_eq!(
		out,
		[
			17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142, 220,
			157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
		]
	);
}

#[test]
fn test_from_str_blake2b_512() {
	let hash = HashImpl::from_str("blake2b_512").unwrap();
	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 64];
	hash.hash(&mut out, &data);
	assert_eq!(
		out.to_vec(),
		vec![
			207, 148, 246, 214, 5, 101, 126, 144, 197, 67, 176, 201, 25, 7, 12, 218, 175, 114, 9,
			197, 225, 234, 88, 172, 184, 243, 86, 143, 162, 17, 66, 104, 220, 154, 195, 186, 254,
			18, 175, 39, 125, 40, 111, 206, 125, 197, 155, 124, 12, 52, 137, 115, 196, 233, 218,
			203, 231, 148, 133, 229, 106, 194, 167, 2,
		]
	);
}

#[test]
fn test_from_str_blake2b_160() {
	let hash = HashImpl::from_str("blake2b_160").unwrap();
	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 20];
	hash.hash(&mut out, &data);
	assert_eq!(
		out,
		[
			197, 117, 145, 134, 122, 108, 242, 5, 233, 74, 212, 142, 167, 139, 236, 142, 103, 194,
			14, 98
		]
	);
}

#[test]
fn test_from_str_sm3() {
	let hash = HashImpl::from_str("sm3").unwrap();
	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 32];
	hash.hash(&mut out, &data);
	assert_eq!(
		out,
		[
			158, 139, 109, 83, 238, 96, 25, 26, 219, 93, 71, 130, 155, 7, 70, 50, 56, 171, 15, 159,
			227, 157, 222, 97, 216, 238, 73, 54, 50, 158, 49, 251
		]
	);
}
