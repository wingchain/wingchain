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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use executor_primitives::{self, Context, EmptyParams, Module, ModuleError, SEPARATOR, Util};
use node_vm::{VMCallEnv, VMContext, VMContextEnv, VMContractEnv};
use node_vm::errors::{ApplicationError, ContractError, VMError, VMResult};
use primitives::{Address, Balance, DBKey, DBValue, Hash};

const CONTRACT_DATA_STORAGE_KEY: &[u8] = b"contract_data";

pub struct DefaultVMContext<M: Module> {
    env: Rc<VMContextEnv>,
    call_env: Rc<VMCallEnv>,
    contract_env: Rc<VMContractEnv>,
    executor_context: M::C,
    executor_util: M::U,
    payload_buffer: RefCell<HashMap<DBKey, Option<DBValue>>>,
    events: RefCell<Vec<Vec<u8>>>,
}

impl<M: Module> DefaultVMContext<M> {
    pub fn new(
        env: Rc<VMContextEnv>,
        call_env: Rc<VMCallEnv>,
        contract_env: Rc<VMContractEnv>,
        executor_context: M::C,
        executor_util: M::U,
    ) -> Self {
        DefaultVMContext {
            env,
            call_env,
            contract_env,
            executor_context,
            executor_util,
            payload_buffer: RefCell::new(HashMap::new()),
            events: RefCell::new(Vec::new()),
        }
    }

    /// Translate key in vm to key in module
    /// [module_key]_[contract_data_storage_key]_hash([contract_address]_[key])
    fn vm_to_module_key(&self, key: &[u8]) -> VMResult<Vec<u8>> {
        let key = &[&self.contract_env.contract_address.0, SEPARATOR, key].concat();
        let key = self.executor_util.hash(key).map_err(Self::module_to_vm_error)?;
        let key = [M::STORAGE_KEY, SEPARATOR, CONTRACT_DATA_STORAGE_KEY, SEPARATOR, &key.0].concat();
        Ok(key)
    }

    fn module_to_vm_error(e: ModuleError) -> VMError {
        match e {
            ModuleError::System(e) => VMError::System(e),
            ModuleError::Application(e) => match e {
                executor_primitives::errors::ApplicationError::InvalidAddress(_) => ContractError::InvalidAddress.into(),
                executor_primitives::errors::ApplicationError::User { msg } => (ContractError::User { msg }).into(),
            },
        }
    }
}

impl<M: Module> VMContext for DefaultVMContext<M> {
    fn env(&self) -> Rc<VMContextEnv> {
        self.env.clone()
    }
    fn call_env(&self) -> Rc<VMCallEnv> {
        self.call_env.clone()
    }
    fn contract_env(&self) -> Rc<VMContractEnv> {
        self.contract_env.clone()
    }
    fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
        if let Some(value) = self.payload_buffer.borrow().get(&DBKey::from_slice(key)) {
            return Ok(value.clone());
        }
        let key = self.vm_to_module_key(key)?;
        let result = self.executor_context.payload_get(&key).map_err(Self::module_to_vm_error)?;
        Ok(result)
    }
    fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
        let mut buffer = self.payload_buffer.borrow_mut();
        buffer.insert(DBKey::from_slice(key), value);
        Ok(())
    }
    fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
        let buffer = self.payload_buffer
            .borrow_mut()
            .drain()
            .collect();
        Ok(buffer)
    }
    fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
        for (key, value) in items {
            let key = &self.vm_to_module_key(key.as_slice())?;
            self.executor_context.payload_set(key, value).map_err(Self::module_to_vm_error)?;
        }
        Ok(())
    }
    fn emit_event(&self, event: Vec<u8>) -> VMResult<()> {
        self.events.borrow_mut().push(event);
        Ok(())
    }
    fn drain_events(&self) -> VMResult<Vec<Vec<u8>>> {
        let mut events = self.events.borrow_mut();
        let events = events.drain(..).collect();
        Ok(events)
    }
    fn hash(&self, data: &[u8]) -> VMResult<Hash> {
        self.executor_util.hash(data).map_err(Self::module_to_vm_error)
    }
    fn address(&self, data: &[u8]) -> VMResult<Address> {
        self.executor_util.address(data).map_err(Self::module_to_vm_error)
    }
    fn validate_address(&self, address: &Address) -> VMResult<()> {
        self.executor_util.validate_address(address).map_err(Self::module_to_vm_error)
    }
    fn balance_get(&self, address: &Address) -> VMResult<Balance> {
        let balance_module = <module_balance::Module<M::C, M::U> as Module>::new(self.executor_context.clone(), self.executor_util.clone());
        let sender = Some(&self.contract_env.sender_address);
        balance_module.get_balance(sender, EmptyParams).map_err(Self::module_to_vm_error)
    }
    fn balance_transfer(
        &self,
        sender: &Address,
        recipient: &Address,
        value: Balance,
    ) -> VMResult<()> {
        unimplemented!()
    }
}
