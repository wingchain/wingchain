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
		IO(std::io::Error) #[doc="IO error"];
		Toml(toml::de::Error) #[doc="Toml error"];
		Codec(parity_codec::Error) #[doc="Codec error"];
	}
	links {
		Crypto(crypto::errors::Error, crypto::errors::ErrorKind) #[doc="Crypto error"];
		DB(node_db::errors::Error, node_db::errors::ErrorKind) #[doc="DB error"];
		StateDB(node_statedb::errors::Error, node_statedb::errors::ErrorKind) #[doc="StateDB error"];
		Executor(node_executor::errors::Error, node_executor::errors::ErrorKind) #[doc="Executor error"];
	}
	errors {
		InvalidSpec {
			description(""),
			display("Invalid spec"),
		}
		DBIntegrityLess(reason: String) {
			description(""),
			display("DB integrity less: {:?}", reason),
		}
		NotReleasedProperly {
			description(""),
			display("Not released properly"),
		}
	}
}
