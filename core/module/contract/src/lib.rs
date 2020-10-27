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

use std::rc::Rc;

use executor_macro::{call, module};
use executor_primitives::{errors, Context, ContextEnv, Module as ModuleT, StorageMap, Validator};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{codec, Address, Balance, Call, TransactionResult};
use std::collections::HashMap;

pub struct Module<C>
where
	C: Context,
{
	#[allow(dead_code)]
	env: Rc<ContextEnv>,
	context: C,
	/// contract address -> admin
	admin: StorageMap<Address, Admin, C>,
	/// contract address -> current version
	version: StorageMap<Address, u32, C>,
	/// (contract address, version) -> code
	code: StorageMap<(Address, u32), Vec<u8>, C>,

	/// contract address -> admin update proposal id
	admin_update_proposal_id: StorageMap<Address, u32, C>,
	/// contract address -> admin update proposal
	admin_update_proposal: StorageMap<Address, AdminUpdateProposal, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8] = b"contract";

	fn new(context: C) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			admin: StorageMap::new::<Self>(context.clone(), b"admin"),
			version: StorageMap::new::<Self>(context.clone(), b"version"),
			code: StorageMap::new::<Self>(context.clone(), b"code"),
			admin_update_proposal_id: StorageMap::new::<Self>(
				context.clone(),
				b"admin_update_proposal_id",
			),
			admin_update_proposal: StorageMap::new::<Self>(context, b"admin_update_proposal"),
		}
	}

	#[call]
	fn get_version(
		&self,
		_sender: Option<&Address>,
		params: GetVersionParams,
	) -> CommonResult<CallResult<Option<u32>>> {
		let contract_address = params.contract_address;
		let version = self.version.get(&contract_address)?;
		Ok(Ok(version))
	}

	#[call]
	fn get_admin(
		&self,
		_sender: Option<&Address>,
		params: GetAdminParams,
	) -> CommonResult<CallResult<Option<Admin>>> {
		let contract_address = params.contract_address;
		let admin = self.admin.get(&contract_address)?;
		Ok(Ok(admin))
	}

	#[call]
	fn get_code(
		&self,
		_sender: Option<&Address>,
		params: GetCodeParams,
	) -> CommonResult<CallResult<Option<Vec<u8>>>> {
		let contract_address = params.contract_address;
		let version = params.version;

		let version = match version {
			Some(version) => version,
			None => {
				let current_version = self.version.get(&contract_address)?;
				match current_version {
					Some(current_version) => current_version,
					None => return Ok(Ok(None)),
				}
			}
		};

		let code = self.code.get(&(contract_address, version))?;
		Ok(Ok(code))
	}

	#[call(write = true)]
	fn create(
		&self,
		sender: Option<&Address>,
		params: CreateParams,
	) -> CommonResult<CallResult<Address>> {
		let sender = sender.expect("should be signed");
		let contract_address = self.context.call_env().unique_address.clone();

		let code = params.code;
		let version = 1u32;
		let admin = Admin {
			threshold: 1,
			members: vec![(sender.clone(), 1)],
		};
		self.version.set(&contract_address, &version)?;
		self.admin.set(&contract_address, &admin)?;
		self.code.set(&(contract_address.clone(), version), &code)?;

		Ok(Ok(contract_address))
	}

	#[call(write = true)]
	fn update_admin(
		&self,
		sender: Option<&Address>,
		params: UpdateAdminParams,
	) -> CommonResult<CallResult<()>> {
		let sender = sender.expect("should be signed");
		let contract_address = params.contract_address;

		// verify sender
		let old_admin = self
			.admin
			.get(&contract_address)?
			.expect("should have admin");
		let old_threshold = old_admin.threshold;
		let old_members = old_admin.members.into_iter().collect::<HashMap<_, _>>();
		if !old_members.contains_key(sender) {
			return Ok(Err("not admin".to_string()));
		}

		// create a proposal
		let new_admin = aggregate_admin(params.admin);
		let proposal_id = self
			.admin_update_proposal_id
			.get(&contract_address)?
			.unwrap_or(1u32);
		let mut proposal = AdminUpdateProposal {
			proposal_id,
			admin: new_admin.clone(),
			vote: vec![],
		};
		let event = UpdateAdminEvent::ProposalCreated(AdminUpdateProposalCreated {
			proposal: proposal.clone(),
		});
		self.context.emit_event(event)?;

		// vote for the proposal
		if !proposal.vote.contains(sender) {
			proposal.vote.push(sender.clone());
		}
		let event = UpdateAdminEvent::ProposalVoted(AdminUpdateProposalVoted {
			proposal: proposal.clone(),
		});
		self.context.emit_event(event)?;

		// pass a proposal
		let sum = proposal
			.vote
			.iter()
			.fold(0u32, |x, v| x + *old_members.get(v).unwrap_or(&0u32));
		let mut pass = false;
		if sum >= old_threshold {
			self.admin.set(&contract_address, &new_admin)?;
			pass = true;

			let event = UpdateAdminEvent::ProposalPassed(AdminUpdateProposalPassed {
				proposal: proposal.clone(),
			});
			self.context.emit_event(event)?;
		}

		if pass {
			self.admin_update_proposal.delete(&contract_address)?;
		} else {
			self.admin_update_proposal
				.set(&contract_address, &proposal)?;
		};
		self.admin_update_proposal_id
			.set(&contract_address, &(proposal_id + 1))?;

		Ok(Ok(()))
	}

	#[call(write = true)]
	fn update_admin_vote(
		&self,
		sender: Option<&Address>,
		params: UpdateAdminVoteParams,
	) -> CommonResult<CallResult<()>> {
		let sender = sender.expect("should be signed");
		let contract_address = params.contract_address;

		let proposal = self.admin_update_proposal.get(&contract_address)?;
		let mut proposal = match proposal {
			Some(proposal) => proposal,
			None => return Ok(Err("not proposal".to_string())),
		};

		if proposal.proposal_id != params.proposal_id {
			return Ok(Err("proposal_id not match".to_string()));
		}

		// verify sender
		let old_admin = self
			.admin
			.get(&contract_address)?
			.expect("should have admin");
		let old_threshold = old_admin.threshold;
		let old_members = old_admin.members.into_iter().collect::<HashMap<_, _>>();
		if !old_members.contains_key(sender) {
			return Ok(Err("not admin".to_string()));
		}

		// vote for the proposal
		if !proposal.vote.contains(sender) {
			proposal.vote.push(sender.clone());
		}
		let event = UpdateAdminEvent::ProposalVoted(AdminUpdateProposalVoted {
			proposal: proposal.clone(),
		});
		self.context.emit_event(event)?;

		// pass a proposal
		let sum = proposal
			.vote
			.iter()
			.fold(0u32, |x, v| x + *old_members.get(v).unwrap_or(&0u32));
		let mut pass = false;
		if sum >= old_threshold {
			self.admin.set(&contract_address, &proposal.admin)?;
			pass = true;

			let event = UpdateAdminEvent::ProposalPassed(AdminUpdateProposalPassed {
				proposal: proposal.clone(),
			});
			self.context.emit_event(event)?;
		}

		if pass {
			self.admin_update_proposal.delete(&contract_address)?;
		} else {
			self.admin_update_proposal
				.set(&contract_address, &proposal)?;
		};

		Ok(Ok(()))
	}

	#[call(write = true)]
	fn execute(
		&self,
		_sender: Option<&Address>,
		_params: ExecuteParams,
	) -> CommonResult<CallResult<Address>> {
		unimplemented!()
	}
}

fn aggregate_admin(admin: Admin) -> Admin {
	let threshold = admin.threshold;
	let members = admin.members;
	let mut new_members = Vec::<(Address, u32)>::new();
	for (address, weight) in members {
		if weight > 0 {
			match new_members.iter().position(|x| &x.0 == &address) {
				Some(position) => {
					let find = new_members.get_mut(position).unwrap();
					find.1 += weight;
				}
				None => new_members.push((address, weight)),
			}
		}
	}
	Admin {
		threshold,
		members: new_members,
	}
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct CreateParams {
	/// wasm code
	pub code: Vec<u8>,
	/// amount sent to contract when creating
	pub value: Balance,
	/// init params in json format
	pub init_params: Vec<u8>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct ExecuteParams {
	/// contract address
	pub contract_address: Address,
	/// amount sent to contract when execute a payable method
	pub value: Balance,
	/// contract method
	pub method: String,
	/// params in json format
	pub params: Vec<u8>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct GetVersionParams {
	/// contract address
	pub contract_address: Address,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct GetAdminParams {
	/// contract address
	pub contract_address: Address,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct GetCodeParams {
	/// contract address
	pub contract_address: Address,
	/// version
	pub version: Option<u32>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone)]
pub struct Admin {
	pub threshold: u32,
	pub members: Vec<(Address, u32)>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone)]
pub struct AdminUpdateProposal {
	pub proposal_id: u32,
	pub admin: Admin,
	pub vote: Vec<Address>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAdminParams {
	/// contract address
	pub contract_address: Address,
	/// admin
	pub admin: Admin,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAdminVoteParams {
	/// contract address
	pub contract_address: Address,
	/// proposal id
	pub proposal_id: u32,
}

#[derive(Encode, Decode, Debug)]
pub enum UpdateAdminEvent {
	ProposalCreated(AdminUpdateProposalCreated),
	ProposalVoted(AdminUpdateProposalVoted),
	ProposalPassed(AdminUpdateProposalPassed),
}

#[derive(Encode, Decode, Debug)]
pub struct AdminUpdateProposalCreated {
	pub proposal: AdminUpdateProposal,
}

#[derive(Encode, Decode, Debug)]
pub struct AdminUpdateProposalVoted {
	pub proposal: AdminUpdateProposal,
}

#[derive(Encode, Decode, Debug)]
pub struct AdminUpdateProposalPassed {
	pub proposal: AdminUpdateProposal,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_aggregate_admin() {
		let admin = Admin {
			threshold: 2,
			members: vec![
				(Address(vec![1, 1, 1, 1]), 2),
				(Address(vec![2, 2, 2, 2]), 3),
				(Address(vec![1, 1, 1, 1]), 4),
				(Address(vec![3, 3, 3, 3]), 0),
			],
		};
		let admin = aggregate_admin(admin);
		assert_eq!(
			admin,
			Admin {
				threshold: 2,
				members: vec![
					(Address(vec![1, 1, 1, 1]), 6),
					(Address(vec![2, 2, 2, 2]), 3)
				],
			}
		)
	}
}
