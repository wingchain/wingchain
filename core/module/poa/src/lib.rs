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

use std::sync::Arc;

use executor_macro::{call, module};
use executor_primitives::{
	errors, errors::ApplicationError, Context, ContextEnv, EmptyParams, Module as ModuleT,
	ModuleResult, OpaqueModuleResult, StorageValue, Util,
};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Call, Event};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	env: Arc<ContextEnv>,
	#[allow(dead_code)]
	context: C,
	util: U,
	block_interval: StorageValue<Option<u64>, Self>,
	admin: StorageValue<Admin, Self>,
	authority: StorageValue<Address, Self>,

	/// update admin proposal id
	update_admin_proposal_id: StorageValue<u32, Self>,
	/// update admin proposal
	update_admin_proposal: StorageValue<UpdateAdminProposal, Self>,

	/// update authority proposal id
	update_authority_proposal_id: StorageValue<u32, Self>,
	/// update authority proposal
	update_authority_proposal: StorageValue<UpdateAuthorityProposal, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"poa";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			util,
			block_interval: StorageValue::new(context.clone(), b"block_interval"),
			admin: StorageValue::new(context.clone(), b"admin"),
			authority: StorageValue::new(context.clone(), b"authority"),
			update_admin_proposal_id: StorageValue::new(
				context.clone(),
				b"update_admin_proposal_id",
			),
			update_admin_proposal: StorageValue::new(context.clone(), b"update_admin_proposal"),
			update_authority_proposal_id: StorageValue::new(
				context.clone(),
				b"update_authority_proposal_id",
			),
			update_authority_proposal: StorageValue::new(context, b"update_authority_proposal"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("Not genesis".into());
		}
		self.block_interval.set(&params.block_interval)?;
		self.admin.set(&params.admin)?;
		self.authority.set(&params.authority)?;
		Ok(())
	}

	fn validate_init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		for (address, _) in &params.admin.members {
			self.util.validate_address(address)?;
		}
		self.util.validate_address(&params.authority)?;
		Ok(())
	}

	#[call]
	fn get_meta(&self, _sender: Option<&Address>, _params: EmptyParams) -> ModuleResult<Meta> {
		let block_interval = self.block_interval.get()?;
		let block_interval = block_interval.ok_or("Unexpected none")?;

		let meta = Meta { block_interval };
		Ok(meta)
	}

	#[call]
	fn get_authority(
		&self,
		_sender: Option<&Address>,
		_params: EmptyParams,
	) -> ModuleResult<Address> {
		let authority = self.authority.get()?;
		let authority = authority.ok_or("Unexpected none")?;
		Ok(authority)
	}

	#[call]
	fn get_admin(&self, _sender: Option<&Address>, _params: EmptyParams) -> ModuleResult<Admin> {
		let admin = self.admin.get()?;
		let admin = admin.ok_or("Unexpected none")?;
		Ok(admin)
	}

	#[call(write = true)]
	fn update_admin(
		&self,
		sender: Option<&Address>,
		params: UpdateAdminParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;

		let (old_threshold, old_members) = self.verify_sender(sender)?;

		// create a proposal
		let new_admin = aggregate_admin(params.admin);
		let proposal_id = self.update_admin_proposal_id.get()?.unwrap_or(1u32);
		let mut proposal = UpdateAdminProposal {
			proposal_id,
			admin: new_admin,
			vote: vec![],
		};
		self.context.emit_event(Event::from_data(
			"UpdateAdminProposalCreated".to_string(),
			UpdateAdminProposalCreated {
				proposal: proposal.clone(),
			},
		)?)?;

		self.update_admin_vote_and_pass(sender, &mut proposal, old_threshold, &old_members)
	}

	#[call(write = true)]
	fn update_admin_vote(
		&self,
		sender: Option<&Address>,
		params: UpdateAdminVoteParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;

		let proposal = self.update_admin_proposal.get()?;
		let mut proposal = proposal.ok_or("Proposal not found")?;

		if proposal.proposal_id != params.proposal_id {
			return Err("Proposal id not match".into());
		}

		let (old_threshold, old_members) = self.verify_sender(sender)?;

		self.update_admin_vote_and_pass(sender, &mut proposal, old_threshold, &old_members)
	}

	#[call(write = true)]
	fn update_authority(
		&self,
		sender: Option<&Address>,
		params: UpdateAuthorityParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;

		let (old_threshold, old_members) = self.verify_sender(sender)?;

		let authority = params.authority;

		// create a proposal
		let proposal_id = self.update_authority_proposal_id.get()?.unwrap_or(1u32);
		let mut proposal = UpdateAuthorityProposal {
			proposal_id,
			authority,
			vote: vec![],
		};

		self.context.emit_event(Event::from_data(
			"UpdateAuthorityProposalCreated".to_string(),
			UpdateAuthorityProposalCreated {
				proposal: proposal.clone(),
			},
		)?)?;

		self.update_authority_vote_and_pass(sender, &mut proposal, old_threshold, &old_members)
	}

	#[call(write = true)]
	fn update_authority_vote(
		&self,
		sender: Option<&Address>,
		params: UpdateAuthorityVoteParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;

		let proposal = self.update_authority_proposal.get()?;
		let mut proposal = proposal.ok_or("Proposal not found")?;

		if proposal.proposal_id != params.proposal_id {
			return Err("Proposal id not match".into());
		}

		let (old_threshold, old_members) = self.verify_sender(sender)?;

		self.update_authority_vote_and_pass(sender, &mut proposal, old_threshold, &old_members)
	}

	fn verify_sender(&self, sender: &Address) -> ModuleResult<(u32, HashMap<Address, u32>)> {
		let admin = self.admin.get()?;
		let admin = admin.ok_or("Admin not found")?;

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
			self.admin.set(&proposal.admin)?;
			pass = true;

			self.context.emit_event(Event::from_data(
				"UpdateAdminProposalPassed".to_string(),
				UpdateAdminProposalPassed {
					proposal: proposal.clone(),
				},
			)?)?;
		}

		if pass {
			self.update_admin_proposal.delete()?;
		} else {
			self.update_admin_proposal.set(&proposal)?;
		};
		self.update_admin_proposal_id
			.set(&(proposal.proposal_id + 1))?;

		Ok(())
	}

	fn update_authority_vote_and_pass(
		&self,
		sender: &Address,
		proposal: &mut UpdateAuthorityProposal,
		old_threshold: u32,
		old_members: &HashMap<Address, u32>,
	) -> ModuleResult<()> {
		// vote for the proposal
		if !proposal.vote.contains(sender) {
			proposal.vote.push(sender.clone());
		}
		self.context.emit_event(Event::from_data(
			"UpdateAuthorityProposalVoted".to_string(),
			UpdateAuthorityProposalVoted {
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
			self.authority.set(&proposal.authority)?;
			pass = true;

			self.context.emit_event(Event::from_data(
				"UpdateAuthorityProposalPassed".to_string(),
				UpdateAuthorityProposalPassed {
					proposal: proposal.clone(),
				},
			)?)?;
		}

		if pass {
			self.update_authority_proposal.delete()?;
		} else {
			self.update_authority_proposal.set(&proposal)?;
		};
		self.update_authority_proposal_id
			.set(&(proposal.proposal_id + 1))?;

		Ok(())
	}
}

fn aggregate_admin(admin: Admin) -> Admin {
	let threshold = admin.threshold;
	let members = admin.members;
	let mut new_members = Vec::<(Address, u32)>::new();
	for (address, weight) in members {
		if weight > 0 {
			match new_members.iter().position(|x| x.0 == address) {
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

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Admin {
	pub threshold: u32,
	pub members: Vec<(Address, u32)>,
}

#[derive(Encode, Decode, Debug, PartialEq, Deserialize)]
pub struct InitParams {
	pub block_interval: Option<u64>,
	pub admin: Admin,
	pub authority: Address,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize)]
pub struct Meta {
	pub block_interval: Option<u64>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UpdateAdminProposal {
	pub proposal_id: u32,
	pub admin: Admin,
	pub vote: Vec<Address>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UpdateAuthorityProposal {
	pub proposal_id: u32,
	pub authority: Address,
	pub vote: Vec<Address>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAdminParams {
	pub admin: Admin,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAdminVoteParams {
	pub proposal_id: u32,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAuthorityParams {
	pub authority: Address,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAuthorityVoteParams {
	pub proposal_id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAdminProposalCreated {
	pub proposal: UpdateAdminProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAdminProposalVoted {
	pub proposal: UpdateAdminProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAdminProposalPassed {
	pub proposal: UpdateAdminProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAuthorityProposalCreated {
	pub proposal: UpdateAuthorityProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAuthorityProposalVoted {
	pub proposal: UpdateAuthorityProposal,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAuthorityProposalPassed {
	pub proposal: UpdateAuthorityProposal,
}
