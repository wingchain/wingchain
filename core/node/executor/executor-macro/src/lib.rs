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

use proc_macro::TokenStream;

use syn::{
	parse_macro_input, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Meta, NestedMeta, Type,
};

use quote::quote;

#[proc_macro_attribute]
pub fn dispatcher(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let ast: syn::DeriveInput = syn::parse(item).unwrap();

	let variants = match ast.data {
		syn::Data::Enum(ref v) => &v.variants,
		_ => panic!("dispatcher only works on Enum"),
	};

	let is_meta_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C>::META_MODULE) }
		})
		.collect::<Vec<_>>();

	let is_valid_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C>::is_valid_call(&call)) }
		})
		.collect::<Vec<_>>();

	let is_write_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C>::is_write_call(&call)) }
		})
		.collect::<Vec<_>>();

	let execute_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => {
				let module = module::#ident::Module::<_>::new(context.clone());
				let result = module.execute_call(sender, call);
				result
			} }
		})
		.collect::<Vec<_>>();

	let type_name = &ast.ident;
	let gen = quote! {
		struct #type_name;
		impl #type_name {
			fn is_meta<C: ContextT>(module: &str) -> CommonResult<bool>{
				match module {
					#(#is_meta_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
			fn is_valid_call<C: ContextT>(module: &str, call: &Call) -> CommonResult<bool>{
				match module {
					#(#is_valid_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
			fn is_write_call<C: ContextT>(module: &str, call: &Call) -> CommonResult<Option<bool>> {
				match module {
					#(#is_write_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
			fn execute_call<C: ContextT>(module: &str, context: &C, sender: Option<&Address>, call: &Call) -> CommonResult<CommonResult<()>>{
				match module {
					#(#execute_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
		}
	};
	gen.into()
}

#[proc_macro_attribute]
pub fn call(_attr: TokenStream, item: TokenStream) -> TokenStream {
	item
}

#[proc_macro_attribute]
pub fn module(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let impl_item = parse_macro_input!(item as ItemImpl);

	let type_name = get_module_ident(&impl_item);

	let methods = get_module_call_methods(&impl_item);

	let is_valid_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			let params_ident = x.params_ident.clone();
			quote! { stringify!(#method_ident) => codec::decode::<#params_ident>(&params).is_ok() }
		})
		.collect::<Vec<_>>();

	let is_write_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			let is_write = x.write;
			quote! { stringify!(#method_ident) => Some(#is_write) }
		})
		.collect::<Vec<_>>();

	let execute_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			quote! { stringify!(#method_ident) => self.#method_ident(sender, codec::decode(&params).map_err(|_| errors::ErrorKind::InvalidParams)?) }
		})
		.collect::<Vec<_>>();

	let gen = quote! {

		#impl_item

		impl<C> ModuleT<C> for #type_name<C> where C: Context {

			const META_MODULE: bool = Self::META_MODULE;
			const STORAGE_KEY: &'static [u8] = Self::STORAGE_KEY;

			fn new(context: C) -> Self {
				Self::new(context)
			}

			fn is_valid_call(call: &Call) -> bool {
				let params = &call.params.0[..];
				match call.method.as_str() {
					#(#is_valid_call_ts_vec),*,
					_ => false,
				}
			}

			fn is_write_call(call: &Call) -> Option<bool> {
				let method = call.method.as_str();
				match method {
					#(#is_write_call_ts_vec),*,
					other => None,
				}
			}

			fn execute_call(&self, sender: Option<&Address>, call: &Call) -> CommonResult<CommonResult<()>> {
				let params = &call.params.0[..];
				let method = call.method.as_str();
				match method {
					#(#execute_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidMethod(other.to_string()).into()),
				}
			}

		}

	};
	gen.into()
}

fn get_module_ident(impl_item: &ItemImpl) -> Ident {
	match &*impl_item.self_ty {
		Type::Path(type_path) => type_path.path.segments[0].ident.clone(),
		_ => panic!("module ident not found"),
	}
}

fn get_module_call_methods(impl_item: &ItemImpl) -> Vec<ModuleMethod> {
	impl_item
		.items
		.iter()
		.filter_map(|item| {
			if let ImplItem::Method(method) = item {
				let call_method = method.attrs.iter().find_map(|attr| {
					let meta = attr.parse_meta().unwrap();
					match meta {
						Meta::Path(word) => {
							if word.segments[0].ident == "call" {
								Some(ModuleMethod {
									write: false,
									method_ident: method.sig.ident.clone(),
									params_ident: get_method_params_ident(&method),
									method: method.clone(),
								})
							} else {
								None
							}
						}
						Meta::List(list) => {
							if list.path.segments[0].ident == "call" {
								let write = list.nested.iter().find_map(|nm| match nm {
									NestedMeta::Meta(Meta::NameValue(nv))
										if nv.path.segments[0].ident == "write" =>
									{
										match &nv.lit {
											syn::Lit::Bool(value) => Some(value.value),
											_ => panic!("write should have a bool value"),
										}
									}
									_ => None,
								});
								let write = write.unwrap_or(false);
								Some(ModuleMethod {
									write,
									method_ident: method.sig.ident.clone(),
									params_ident: get_method_params_ident(&method),
									method: method.clone(),
								})
							} else {
								None
							}
						}
						_ => None,
					}
				});
				call_method
			} else {
				None
			}
		})
		.collect()
}

fn get_method_params_ident(method: &ImplItemMethod) -> Ident {
	if method.sig.inputs.len() == 3 {
		let params = &method.sig.inputs[2];
		let pat_type = match params {
			FnArg::Typed(pat_type) => pat_type,
			_ => unreachable!(),
		};
		let params_ident = if let Type::Path(path) = &*pat_type.ty {
			path.path
				.get_ident()
				.expect("no params ident found")
				.clone()
		} else {
			panic!("no params type found")
		};
		params_ident
	} else {
		panic!("call method input should be (&self, params: Type)");
	}
}

struct ModuleMethod {
	#[allow(dead_code)]
	method: ImplItemMethod,
	method_ident: Ident,
	params_ident: Ident,
	write: bool,
}
