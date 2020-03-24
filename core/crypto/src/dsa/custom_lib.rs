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

use libloading::Library;

pub trait CDsa {
	const ERR_LEN: usize;
	type KeyPair: CKeyPair;
	type Verifier: CVerifier;

	fn name(&self) -> String;

	fn generate_key_pair(&self) -> Result<Self::KeyPair, Vec<u8>>;

	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> Result<Self::KeyPair, Vec<u8>>;

	fn verifier_from_public_key(&self, public_key: &[u8]) -> Result<Self::Verifier, Vec<u8>>;
}

pub trait CKeyPair {
	const PUBLIC_LEN: usize;
	const SECRET_LEN: usize;
	const SIGNATURE_LEN: usize;
	fn public_key(&self, out: &mut [u8]);
	fn secret_key(&self, out: &mut [u8]);
	fn sign(&self, message: &[u8], out: &mut [u8]);
}

pub trait CVerifier {
	const ERR_LEN: usize;
	fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Vec<u8>>;
}

pub struct CustomLib {
	#[allow(dead_code)]
	/// lib is referred by symbols, should be kept
	lib: Library,
	name: String,
	//call_generate_key_pair: imp::Symbol<CallHash>,
}
