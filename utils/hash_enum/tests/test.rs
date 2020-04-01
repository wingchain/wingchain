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

use hash_enum::HashEnum;
use hash_enum_derive::HashEnum;
use crypto::blake2b;

#[test]
fn test_hash_enum_derive() {
	#[derive(HashEnum, PartialEq, Debug)]
	enum Test {
		Foo,
		Bar,
	}

	let test = Test::Bar;
	assert_eq!(test.name(), "bar");
	assert_eq!(test.hash(), &blake2b256(b"bar")[0..4]);
	assert_eq!(Test::from_name("bar"), Some(Test::Bar));

	let hash = &blake2b256(b"bar")[0..4];
	assert_eq!(Test::from_hash(hash), Some(Test::Bar));
}

fn blake2b256(data: &[u8]) -> [u8; 32] {
	let mut out = [0u8; 32];
	blake2b::Blake2b::blake2b(&mut out, &data, &[]);
	out
}
