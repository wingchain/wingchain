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

use std::fmt::Debug;

use error_chain::*;
use trie_db::TrieError;

error_chain! {
	foreign_links {
		MutStatic(mut_static::error::Error) #[doc="Mut static error"];
	}
	links {
		DB(node_db::errors::Error, node_db::errors::ErrorKind) #[doc="DB error"];
	}
	errors {
		LoadHasherConflict(old: String, new: String) {
			description(""),
			display("Load hasher conflict: {}, {}", old, new),
		}
	}
}

pub fn parse_trie_error<T, E>(e: Box<TrieError<T, E>>) -> String
where
	T: Debug,
	E: Debug,
{
	format!("{}", e)
}
