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

use error_chain::*;

error_chain! {
	foreign_links {
	}
	links {
	}
	errors {
		HashNameNotFound(name: String) {
			description(""),
			display("Hash name not found: {}", name),
		}
		CustomLibNotFound(path: String) {
			description(""),
			display("Custom lib not found: {}", path),
		}
		CustomLibLoadFailed(path: String) {
			description(""),
			display("Custom lib load failed: {}", path),
		}
		InvalidKeyLength(length: usize) {
			description(""),
			display("Invalid key length: {}", length),
		}
		InvalidName(path: String) {
			description(""),
			display("Invalid name: {}", path),
		}
		InvalidSecretKey {
			description(""),
			display("Invalid public key"),
		}
		InvalidPublicKey {
			description(""),
			display("Invalid public key"),
		}
		VerificationFailed {
			description(""),
			display("Verification failed"),
		}
		InvalidAddressLength(length: usize) {
			description(""),
			display("Invalid address length: {}", length),
		}
	}
}
