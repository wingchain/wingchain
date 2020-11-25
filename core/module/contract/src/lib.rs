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

use std::collections::HashMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use executor_macro::{call, module};
use executor_primitives::{
	errors, errors::ApplicationError, Context, ContextEnv, Module as ModuleT, ModuleError,
	ModuleResult, OpaqueModuleResult, StorageMap, Util,
};
use node_vm::errors::VMError;
use node_vm::{Mode, VMConfig, VMContractEnv, VM};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Balance, Call, Event, Hash};

use crate::vm::DefaultVMContext;

mod vm;

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	#[allow(dead_code)]
	env: Rc<ContextEnv>,
	context: C,
	util: U,
	/// contract address -> admin
	admin: StorageMap<Address, Admin, Self>,
	/// contract address -> current version
	version: StorageMap<Address, u32, Self>,
	/// (contract address, version) -> code
	code: StorageMap<(Address, u32), Vec<u8>, Self>,
	/// (contract address, version) -> code hash
	code_hash: StorageMap<(Address, u32), Hash, Self>,

	/// contract address -> update admin proposal id
	update_admin_proposal_id: StorageMap<Address, u32, Self>,
	/// contract address -> update admin proposal
	update_admin_proposal: StorageMap<Address, UpdateAdminProposal, Self>,

	/// contract address -> update code proposal id
	update_code_proposal_id: StorageMap<Address, u32, Self>,
	/// contract address -> update code proposal
	update_code_proposal: StorageMap<Address, UpdateCodeProposal, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8] = b"contract";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			util,
			admin: StorageMap::new(context.clone(), b"admin"),
			version: StorageMap::new(context.clone(), b"version"),
			code: StorageMap::new(context.clone(), b"code"),
			code_hash: StorageMap::new(context.clone(), b"code_hash"),
			update_admin_proposal_id: StorageMap::new(context.clone(), b"update_admin_proposal_id"),
			update_admin_proposal: StorageMap::new(context.clone(), b"update_admin_proposal"),
			update_code_proposal_id: StorageMap::new(context.clone(), b"update_code_proposal_id"),
			update_code_proposal: StorageMap::new(context, b"update_code_proposal"),
		}
	}

	#[call]
	fn get_version(
		&self,
		_sender: Option<&Address>,
		params: GetVersionParams,
	) -> ModuleResult<Option<u32>> {
		let contract_address = params.contract_address;
		let version = self.version.get(&contract_address)?;
		Ok(version)
	}

	#[call]
	fn get_admin(
		&self,
		_sender: Option<&Address>,
		params: GetAdminParams,
	) -> ModuleResult<Option<Admin>> {
		let contract_address = params.contract_address;
		let admin = self.admin.get(&contract_address)?;
		Ok(admin)
	}

	#[call]
	fn get_code(
		&self,
		_sender: Option<&Address>,
		params: GetCodeParams,
	) -> ModuleResult<Option<Vec<u8>>> {
		let contract_address = params.contract_address;
		let version = params.version;
		self.inner_get_code(contract_address, version)
	}

	#[call]
	fn get_code_hash(
		&self,
		_sender: Option<&Address>,
		params: GetCodeHashParams,
	) -> ModuleResult<Option<Hash>> {
		let contract_address = params.contract_address;
		let version = params.version;

		let version = match version {
			Some(version) => version,
			None => {
				let current_version = self.version.get(&contract_address)?;
				match current_version {
					Some(current_version) => current_version,
					None => return Ok(None),
				}
			}
		};

		let code_hash = self.code_hash.get(&(contract_address, version))?;
		Ok(code_hash)
	}

	fn validate_create(util: &U, params: CreateParams) -> ModuleResult<()> {
		let vm = VM::new(VMConfig::default());
		let code = params.code;
		let code_hash = util.hash(&code)?;
		let init_method = params.init_method;
		let init_params = params.init_params;
		let init_pay_value = params.init_pay_value;
		vm.validate(
			&code_hash,
			&code,
			Mode::Init,
			&init_method,
			&init_params,
			init_pay_value,
		)
		.map_err(vm_to_module_error)?;

		Ok(())
	}

	#[call(write = true)]
	fn create(&self, sender: Option<&Address>, params: CreateParams) -> ModuleResult<Address> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;
		let tx_hash = &self.context.call_env().tx_hash;
		let tx_hash = tx_hash.as_ref().ok_or("Tx hash not found")?;
		let contract_address = self.util.address(&self.util.hash(&tx_hash.0)?.0)?;

		let code = params.code;
		let code_hash = self.util.hash(&code)?;
		let version = 1u32;
		let admin = Admin {
			threshold: 1,
			members: vec![(sender.clone(), 1)],
		};
		self.version.set(&contract_address, &version)?;
		self.admin.set(&contract_address, &admin)?;
		self.code.set(&(contract_address.clone(), version), &code)?;
		self.code_hash
			.set(&(contract_address.clone(), version), &code_hash)?;

		Ok(contract_address)
	}

	fn validate_update_admin(util: &U, params: UpdateAdminParams) -> ModuleResult<()> {
		for (address, _) in params.admin.members {
			util.validate_address(&address)?;
		}
		Ok(())
	}

	#[call(write = true)]
	fn update_admin(
		&self,
		sender: Option<&Address>,
		params: UpdateAdminParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;
		let contract_address = params.contract_address;

		let (old_threshold, old_members) = self.verify_sender(sender, &contract_address)?;

		// create a proposal
		let new_admin = aggregate_admin(params.admin);
		let proposal_id = self
			.update_admin_proposal_id
			.get(&contract_address)?
			.unwrap_or(1u32);
		let mut proposal = UpdateAdminProposal {
			proposal_id,
			admin: new_admin.clone(),
			vote: vec![],
		};
		self.context.emit_event(Event::from_data(
			"UpdateAdminProposalCreated".to_string(),
			UpdateAdminProposalCreated {
				contract_address: contract_address.clone(),
				proposal: proposal.clone(),
			},
		)?)?;

		self.update_admin_vote_and_pass(
			sender,
			&contract_address,
			&mut proposal,
			old_threshold,
			&old_members,
		)
	}

	#[call(write = true)]
	fn update_admin_vote(
		&self,
		sender: Option<&Address>,
		params: UpdateAdminVoteParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;
		let contract_address = params.contract_address;

		let proposal = self.update_admin_proposal.get(&contract_address)?;
		let mut proposal = proposal.ok_or("Proposal not found")?;

		if proposal.proposal_id != params.proposal_id {
			return Err("Proposal id not match".into());
		}

		let (old_threshold, old_members) = self.verify_sender(sender, &contract_address)?;

		self.update_admin_vote_and_pass(
			sender,
			&contract_address,
			&mut proposal,
			old_threshold,
			&old_members,
		)
	}

	#[call(write = true)]
	fn update_code(&self, sender: Option<&Address>, params: UpdateCodeParams) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;
		let contract_address = params.contract_address;

		let (old_threshold, old_members) = self.verify_sender(sender, &contract_address)?;

		let code = params.code;

		// create a proposal
		let proposal_id = self
			.update_code_proposal_id
			.get(&contract_address)?
			.unwrap_or(1u32);
		let code_hash = self.util.hash(&code)?;
		let mut proposal = UpdateCodeProposal {
			proposal_id,
			code,
			vote: vec![],
		};

		self.context.emit_event(Event::from_data(
			"UpdateCodeProposalCreated".to_string(),
			UpdateCodeProposalCreated {
				contract_address: contract_address.clone(),
				proposal: UpdateCodeProposalForEvent::from(&proposal, &code_hash),
			},
		)?)?;

		self.update_code_vote_and_pass(
			sender,
			&contract_address,
			&mut proposal,
			&code_hash,
			old_threshold,
			&old_members,
		)
	}

	#[call(write = true)]
	fn update_code_vote(
		&self,
		sender: Option<&Address>,
		params: UpdateCodeVoteParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;
		let contract_address = params.contract_address;

		let proposal = self.update_code_proposal.get(&contract_address)?;
		let mut proposal = proposal.ok_or("Proposal not found")?;

		if proposal.proposal_id != params.proposal_id {
			return Err("Proposal id not match".into());
		}

		let (old_threshold, old_members) = self.verify_sender(sender, &contract_address)?;

		let code_hash = self.util.hash(&proposal.code)?;

		self.update_code_vote_and_pass(
			sender,
			&contract_address,
			&mut proposal,
			&code_hash,
			old_threshold,
			&old_members,
		)
	}

	#[call(write = true)]
	fn execute(&self, sender: Option<&Address>, params: ExecuteParams) -> ModuleResult<Vec<u8>> {
		let contract_address = params.contract_address;
		let code = self
			.inner_get_code(contract_address.clone(), None)?
			.ok_or("Contract not found")?;

		let contract_env = Rc::new(VMContractEnv {
			contract_address,
			sender_address: sender.cloned(),
		});
		let vm_context =
			DefaultVMContext::<Self>::new(contract_env, self.context.clone(), self.util.clone());
		let vm = VM::new(VMConfig::default());
		let code_hash = self.util.hash(&code)?;

		let contract_method = params.method;
		let contract_params = params.params;
		let contract_pay_value = params.pay_value;

		let result = vm
			.execute(
				&code_hash,
				&code,
				&vm_context,
				Mode::Call,
				&contract_method,
				&contract_params,
				contract_pay_value,
			)
			.map_err(vm_to_module_error)?;
		Ok(result)
	}

	fn verify_sender(
		&self,
		sender: &Address,
		contract_address: &Address,
	) -> ModuleResult<(u32, HashMap<Address, u32>)> {
		let admin = self.admin.get(&contract_address)?;
		let admin = admin.ok_or("Contract admin not found")?;

		let threshold = admin.threshold;
		let members = admin.members.into_iter().collect::<HashMap<_, _>>();
		if !members.contains_key(sender) {
			return Err("Not admin".into());
		}

		Ok((threshold, members))
	}

	fn update_admin_vote_and_pass(
		&self,
		sender: &Address,
		contract_address: &Address,
		proposal: &mut UpdateAdminProposal,
		old_threshold: u32,
		old_members: &HashMap<Address, u32>,
	) -> ModuleResult<()> {
		// vote for the proposal
		if !proposal.vote.contains(sender) {
			proposal.vote.push(sender.clone());
		}
		self.context.emit_event(Event::from_data(
			"UpdateAdminProposalVoted".to_string(),
			UpdateAdminProposalVoted {
				contract_address: contract_address.clone(),
				proposal: proposal.clone(),
			},
		)?)?;

		// pass a proposal
		let sum = proposal
			.vote
			.iter()
			.fold(0u32, |x, v| x + *old_members.get(v).unwrap_or(&0u32));
		let mut pass = false;
		if sum >= old_threshold {
			self.admin.set(&contract_address, &proposal.admin)?;
			pass = true;

			self.context.emit_event(Event::from_data(
				"UpdateAdminProposalPassed".to_string(),
				UpdateAdminProposalPassed {
					contract_address: contract_address.clone(),
					proposal: proposal.clone(),
				},
			)?)?;
		}

		if pass {
			self.update_admin_proposal.delete(&contract_address)?;
		} else {
			self.update_admin_proposal
				.set(&contract_address, &proposal)?;
		};
		self.update_admin_proposal_id
			.set(&contract_address, &(proposal.proposal_id + 1))?;

		Ok(())
	}

	fn update_code_vote_and_pass(
		&self,
		sender: &Address,
		contract_address: &Address,
		proposal: &mut UpdateCodeProposal,
		code_hash: &Hash,
		old_threshold: u32,
		old_members: &HashMap<Address, u32>,
	) -> ModuleResult<()> {
		// vote for the proposal
		if !proposal.vote.contains(sender) {
			proposal.vote.push(sender.clone());
		}
		self.context.emit_event(Event::from_data(
			"UpdateCodeProposalVoted".to_string(),
			UpdateCodeProposalVoted {
				contract_address: contract_address.clone(),
				proposal: UpdateCodeProposalForEvent::from(proposal, code_hash),
			},
		)?)?;

		// pass a proposal
		let sum = proposal
			.vote
			.iter()
			.fold(0u32, |x, v| x + *old_members.get(v).unwrap_or(&0u32));
		let mut pass = false;
		if sum >= old_threshold {
			let version = self.version.get(&contract_address)?;
			let version = version.ok_or("Contract version not found")?;

			let new_version = version + 1;
			self.version.set(&contract_address, &new_version)?;
			self.code
				.set(&(contract_address.clone(), new_version), &proposal.code)?;
			self.code_hash
				.set(&(contract_address.clone(), new_version), &code_hash)?;
			pass = true;

			self.context.emit_event(Event::from_data(
				"UpdateCodeProposalPassed".to_string(),
				UpdateCodeProposalPassed {
					contract_address: contract_address.clone(),
					proposal: UpdateCodeProposalForEvent::from(proposal, code_hash),
				},
			)?)?;
		}

		if pass {
			self.update_code_proposal.delete(&contract_address)?;
		} else {
			self.update_code_proposal
				.set(&contract_address, &proposal)?;
		};
		self.update_code_proposal_id
			.set(&contract_address, &(proposal.proposal_id + 1))?;

		Ok(())
	}

	fn inner_get_code(
		&self,
		contract_address: Address,
		version: Option<u32>,
	) -> ModuleResult<Option<Vec<u8>>> {
		let version = match version {
			Some(version) => version,
			None => {
				let current_version = self.version.get(&contract_address)?;
				match current_version {
					Some(current_version) => current_version,
					None => return Ok(None),
				}
			}
		};

		let code = self.code.get(&(contract_address, version))?;
		Ok(code)
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

fn vm_to_module_error(e: VMError) -> ModuleError {
	match e {
		VMError::System(e) => ModuleError::System(e),
		VMError::Application(e) => {
			ModuleError::Application(ApplicationError::User { msg: e.to_string() })
		}
	}
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct CreateParams {
	/// wasm code
	pub code: Vec<u8>,
	/// init method
	pub init_method: String,
	/// init params in json format
	pub init_params: Vec<u8>,
	/// amount sent to contract when init
	pub init_pay_value: Balance,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct ExecuteParams {
	/// contract address
	pub contract_address: Address,
	/// contract method
	pub method: String,
	/// params in json format
	pub params: Vec<u8>,
	/// amount sent to contract when execute a payable method
	pub pay_value: Balance,
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

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct GetCodeHashParams {
	/// contract address
	pub contract_address: Address,
	/// version
	pub version: Option<u32>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Admin {
	pub threshold: u32,
	pub members: Vec<(Address, u32)>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UpdateAdminProposal {
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

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateCodeParams {
	/// contract address
	pub contract_address: Address,
	/// wasm code
	pub code: Vec<u8>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateCodeVoteParams {
	/// contract address
	pub contract_address: Address,
	/// proposal id
	pub proposal_id: u32,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone)]
pub struct UpdateCodeProposal {
	pub proposal_id: u32,
	pub code: Vec<u8>,
	pub vote: Vec<Address>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UpdateCodeProposalForEvent {
	pub proposal_id: u32,
	pub code_hash: Hash,
	pub vote: Vec<Address>,
}

impl UpdateCodeProposalForEvent {
	fn from(proposal: &UpdateCodeProposal, code_hash: &Hash) -> Self {
		Self {
			proposal_id: proposal.proposal_id.clone(),
			code_hash: code_hash.clone(),
			vote: proposal.vote.clone(),
		}
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAdminProposalCreated {
	pub contract_address: Address,
	pub proposal: UpdateAdminProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAdminProposalVoted {
	pub contract_address: Address,
	pub proposal: UpdateAdminProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAdminProposalPassed {
	pub contract_address: Address,
	pub proposal: UpdateAdminProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateCodeProposalCreated {
	pub contract_address: Address,
	pub proposal: UpdateCodeProposalForEvent,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateCodeProposalVoted {
	pub contract_address: Address,
	pub proposal: UpdateCodeProposalForEvent,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateCodeProposalPassed {
	pub contract_address: Address,
	pub proposal: UpdateCodeProposalForEvent,
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
