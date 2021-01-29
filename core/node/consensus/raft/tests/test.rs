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

use derive_more::{From, TryInto};
use primitives::codec::{Decode, Encode};
use std::convert::TryInto;
use utils_enum_codec::enum_codec;

#[test]
fn test_enum_codec() {
	#[derive(Encode, Decode, Debug, PartialEq)]
	struct A {
		a: String,
	}

	#[derive(Encode, Decode, Debug, PartialEq)]
	struct B {
		b: u32,
	}

	#[enum_codec]
	#[derive(TryInto, From, Debug, PartialEq)]
	enum E {
		A(A),
		B(B),
	}

	let a: E = A {
		a: "test".to_string(),
	}
	.into();
	let a = a.encode();
	assert_eq!(a, vec![4, 65, 16, 116, 101, 115, 116]);
	let a: E = Decode::decode(&mut &a[..]).unwrap();
	let a: A = a.try_into().unwrap();
	assert_eq!(
		a,
		A {
			a: "test".to_string()
		}
	);

	let b: E = B { b: 10 }.into();
	let b = b.encode();
	assert_eq!(b, vec![4, 66, 10, 0, 0, 0]);
	let b: E = Decode::decode(&mut &b[..]).unwrap();
	let b: B = b.try_into().unwrap();
	assert_eq!(b, B { b: 10 });
}
