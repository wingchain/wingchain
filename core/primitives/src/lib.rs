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

pub mod traits;

pub struct Address(pub Vec<u8>);

/// signature for (nonce, call)
pub struct Signature(pub Vec<u8>);

pub type Nonce = u32;

pub struct Witness {
	address: Address,
	signature: Signature,
	nonce: Nonce,
}

/// sliced digest of module name
pub type ModuleId = [u8; 4];

/// sliced digest of method name
pub type MethodId = [u8; 4];

pub struct Params(pub Vec<u8>);

pub struct Call {
	module_id: ModuleId,
	method_id: MethodId,
	params: Params,
}

pub struct Transaction {
	witness: Option<Witness>,
	call: Call,
}

pub struct Hash(pub Vec<u8>);

pub type BlockNumber = u32;
