#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// https://substrate.dev/docs/en/knowledgebase/runtime/frame

extern crate alloc;

extern crate untrusted;
extern crate base64;
extern crate itertools;
extern crate hex;

extern crate webpki;

use frame_support::{ensure, decl_module, decl_storage, decl_event, decl_error, dispatch};
use frame_system::{ensure_signed, ensure_root};

use alloc::vec::Vec;
use sp_runtime::{traits::AccountIdConversion, ModuleId, SaturatedConversion};
use frame_support::{
	traits::{Currency, ExistenceRequirement::AllowDeath, UnixTime},
};
use codec::{Encode, Decode};
use sp_std::prelude::*;
use secp256k1;

mod hashing;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type BalanceOf<T> = <<T as Trait>::TEECurrency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
const PALLET_ID: ModuleId = ModuleId(*b"Phala!!!");
const BUILTIN_MACHINE_ID: &'static str = "BUILTIN";

#[derive(Encode, Decode)]
pub struct Transfer<AccountId, Balance> {
	pub dest: AccountId,
	pub amount: Balance,
	pub sequence: u64,
}

pub trait SignedDataType<T> {
	fn raw_data(&self) -> Vec<u8>;
	fn signature(&self) -> T;
}

#[derive(Encode, Decode)]
pub struct TransferData<AccountId, Balance> {
	pub data: Transfer<AccountId, Balance>,
	pub signature: Vec<u8>,
}

impl<AccountId: Encode, Balance: Encode> SignedDataType<Vec<u8>> for TransferData<AccountId, Balance> {
	fn raw_data(&self) -> Vec<u8> {
		Encode::encode(&self.data)
	}

	fn signature(&self) -> Vec<u8> {
		self.signature.clone()
	}
}

#[derive(Encode, Decode)]
pub struct Heartbeat {
	block_num: u32,
}

#[derive(Encode, Decode)]
pub struct HeartbeatData {
	data: Heartbeat,
	signature: Vec<u8>,
}

impl SignedDataType<Vec<u8>> for HeartbeatData {
	fn raw_data(&self) -> Vec<u8> {
		Encode::encode(&self.data)
	}

	fn signature(&self) -> Vec<u8> {
		self.signature.clone()
	}
}

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait: frame_system::Trait {
	/// Because this pallet emits events, it depends on the runtime's definition of an event.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

	type TEECurrency: Currency<Self::AccountId>;
	type UnixTime: UnixTime;
}

decl_storage! {
	trait Store for Module<T: Trait> as PhalaModule {
		// Messaging
		/// Number of all commands
		CommandNumber get(fn command_number): Option<u64>;
		/// Contract assignment
		ContractAssign get(fn contract_assign): map hasher(twox_64_concat) u32 => T::AccountId;
		/// Ingress message queue
		IngressSequence get(fn ingress_sequence): map hasher(twox_64_concat) u32 => u64;

		// Worker registry
		/// Map from stash account to worker info (indexed: MachineOwner)
		WorkerState get(fn worker_state): map hasher(blake2_128_concat) T::AccountId => WorkerInfo;
		/// Map from stash account to stash info (indexed: Stash)
		StashState get(fn stash_state): map hasher(blake2_128_concat) T::AccountId => StashInfo<T::AccountId>;

		// Indices
		/// Map from machine_id to stash
		MachineOwner get(fn machine_owner): map hasher(blake2_128_concat) Vec<u8> => T::AccountId;
		/// Map from controller to stash
		Stash get(fn stash): map hasher(blake2_128_concat) T::AccountId => T::AccountId;

		// Key Management
		/// Map from contract id to contract public key (TODO: migrate to real contract key from
		/// worker identity key)
		ContractKey get(fn contract_key): map hasher(twox_64_concat) u32 => Vec<u8>;
	}

	add_extra_genesis {
		config(stakers): Vec<(T::AccountId, T::AccountId, Vec<u8>)>;  // <stash, controller, pubkey>
		config(contract_keys): Vec<Vec<u8>>;
		build(|config: &GenesisConfig<T>| {
			let base_mid = BUILTIN_MACHINE_ID.as_bytes().to_vec();
			for (i, (stash, controller, pubkey)) in config.stakers.iter().enumerate() {
				// Mock worker / stash info
				let mut machine_id = base_mid.clone();
				machine_id.push(b'0' + (i as u8));
				let worker_info = WorkerInfo {
					machine_id,
					pubkey: pubkey.clone(),
					..Default::default()
				};
				WorkerState::<T>::insert(&stash, worker_info);
				let stash_info = StashInfo {
					controller: controller.clone(),
					payout_prefs: PayoutPrefs {
						commission: 0,
						target: stash.clone(),
					}
				};
				StashState::<T>::insert(&stash, stash_info);
				// Update indices (skip MachineOwenr because we won't use it in anyway)
				Stash::<T>::insert(&controller, &stash);
			}
			// Insert the default contract key here
			for (i, key) in config.contract_keys.iter().enumerate() {
				ContractKey::insert(i as u32, key);
			}
		});
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId, Balance = BalanceOf<T> {
		// Debug events
		LogString(Vec<u8>),
		LogI32(i32),
		// Chain events
		CommandPushed(AccountId, u32, Vec<u8>, u64),
		TransferToTee(Vec<u8>, Balance),
		TransferToChain(Vec<u8>, Balance, u64),
		WorkerRegistered(AccountId, Vec<u8>),
		WorkerUnregistered(AccountId, Vec<u8>),
		Heartbeat(AccountId, u32),
	}
);

// Errors inform users that something went wrong.
decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidIASSigningCert,
		InvalidIASReportSignature,
		InvalidQuoteStatus,
		InvalidRuntimeInfo,
		InvalidRuntimeInfoHash,
		MinerNotFound,
		BadMachineId,
		InvalidPubKey,
		InvalidSignature,
		FailedToVerify,
		/// Not a controller account.
		NotController,
		/// Not a stash account.
		NotStash,
		/// Controller not found
		ControllerNotFound,
		/// Stash not found
		StashNotFound,
		/// Stash already bonded
		AlreadyBonded,
		/// Controller already paired
		AlreadyPaired,
		/// Commission is not between 0 and 100
		InvalidCommission,
		// Messagging
		/// Cannot decode the message
		InvalidMessage,
		/// Wrong sequence number of a message
		BadMessageSequence,
		// Token
		/// Failed to deposit tokens to pRuntime due to some internal errors in `Currency` module
		CannotDeposit,
		/// Failed to withdraw tokens from pRuntime reservation due to some internal error in
		/// `Currency` module
		CannotWithdraw,
		/// Bad input parameter
		InvalidInput,
		/// Invalid contract
		InvalidContract,
		/// Internal Error
		InternalError,
	}
}

#[derive(Encode, Decode, Default)]
pub struct WorkerInfo {
	// identity
	pub machine_id: Vec<u8>,
	pub pubkey: Vec<u8>,
	pub last_updated: u64,
	// contract
	// ...
	// mining
	pub status: i32,
	// preformance
	pub score: Option<Score>
}

#[derive(Encode, Decode, Default)]
pub struct StashInfo<AccountId: Default> {
	pub controller: AccountId,
	pub payout_prefs: PayoutPrefs::<AccountId>,
}

#[derive(Encode, Decode, Default)]
pub struct PayoutPrefs<AccountId: Default> {
	pub commission: u32,
	pub target: AccountId,
}

#[derive(Encode, Decode, Default)]
pub struct Score {
	pub overall_score: u32,
	pub features: Vec<u32>
}

type MachineId = [u8; 16];
type WorkerPublicKey = [u8; 33];
#[derive(Encode, Decode)]
struct PRuntimeInfo {
	pub version: u8,
	pub machine_id: MachineId,
	pub pubkey: WorkerPublicKey,
	pub features: Vec<u32>
}

// Dispatchable functions allows users to interact with the pallet and invoke state changes.
// These functions materialize as "extrinsics", which are often compared to transactions.
// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		// Messaging

		#[weight = 0]
		pub fn push_command(origin, contract_id: u32, payload: Vec<u8>) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			let num = Self::command_number().unwrap_or(0);
			CommandNumber::put(num + 1);
			Self::deposit_event(RawEvent::CommandPushed(who, contract_id, payload, num));
			Ok(())
		}

		// Registry
		/// Crerate a new stash or update an existing one.
		#[weight = 0]
		pub fn set_stash(origin, controller: T::AccountId) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!Stash::<T>::contains_key(&controller), Error::<T>::AlreadyPaired);
			ensure!(!StashState::<T>::contains_key(&controller), Error::<T>::AlreadyBonded);
			let stash_state = if StashState::<T>::contains_key(&who) {
				// Remove previous controller
				let prev = StashState::<T>::get(&who);
				Stash::<T>::remove(&prev.controller);
				StashInfo {
					controller: controller.clone(),
					..prev
				}
			} else {
				StashInfo {
					controller: controller.clone(),
					payout_prefs: PayoutPrefs {
						commission: 0,
						target: who.clone(),
					}
				}
			};
			StashState::<T>::insert(&who, stash_state);
			Stash::<T>::insert(&controller, who);
			Ok(())
		}

		/// Update the payout preferences. Must be called by the controller.
		#[weight = 0]
		pub fn set_payout_prefs(origin, payout_commission: Option<u32>,
							    payout_target: Option<T::AccountId>)
						        -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Stash::<T>::contains_key(who.clone()), Error::<T>::NotController);
			let stash = Stash::<T>::get(who.clone());
			ensure!(StashState::<T>::contains_key(&stash), Error::<T>::StashNotFound);
			let mut stash_info = StashState::<T>::get(&stash);
			if let Some(val) = payout_commission {
				ensure!(val <= 100, Error::<T>::InvalidCommission);
				stash_info.payout_prefs.commission = val;
			}
			if let Some(val) = payout_target {
				stash_info.payout_prefs.target = val;
			}
			StashState::<T>::insert(&stash, stash_info);
			Ok(())
		}

		/// Register a worker node with a valid Remote Attestation report
		#[weight = 0]
		pub fn register_worker(origin, encoded_runtime_info: Vec<u8>, report: Vec<u8>, signature: Vec<u8>, raw_signing_cert: Vec<u8>) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Stash::<T>::contains_key(&who), Error::<T>::NotController);
			let stash = Stash::<T>::get(&who);
			// Validate report
			let sig_cert = webpki::EndEntityCert::from(&raw_signing_cert);
			ensure!(sig_cert.is_ok(), Error::<T>::InvalidIASSigningCert);
			let sig_cert = sig_cert.unwrap();
			let verify_result = sig_cert.verify_signature(
				&webpki::RSA_PKCS1_2048_8192_SHA256,
				&report,
				&signature
			);
			ensure!(verify_result.is_ok(), Error::<T>::InvalidIASSigningCert);
			// TODO: Validate certificate
			// let chain: Vec<&[u8]> = Vec::new();
			// let now_func = webpki::Time::from_seconds_since_unix_epoch(1573419050);
			// match sig_cert.verify_is_valid_tls_server_cert(
			// 	SUPPORTED_SIG_ALGS,
			// 	&IAS_SERVER_ROOTS,
			// 	&chain,
			// 	now_func
			// ) {
			// 	Ok(()) => (),
			// 	Err(_) => panic!("verify cert failed")
			// };

			// Validate related fields
			let parsed_report: serde_json_no_std::Value = serde_json_no_std::from_slice(&report).unwrap();
			ensure!(
				&parsed_report["isvEnclaveQuoteStatus"] == "OK" || &parsed_report["isvEnclaveQuoteStatus"] == "CONFIGURATION_NEEDED" || &parsed_report["isvEnclaveQuoteStatus"] == "GROUP_OUT_OF_DATE",
				Error::<T>::InvalidQuoteStatus
			);
			// Extract quote fields
			let raw_quote_body = parsed_report["isvEnclaveQuoteBody"].as_str().unwrap();
			let quote_body = base64::decode(&raw_quote_body).unwrap();
			// TODO: check the following fields
			// let mr_enclave = &quote_body[112..143];
			// let isv_prod_id = &quote_body[304..305];
			// let isv_svn = &quote_body[306..307];
			let report_data = &quote_body[368..432];
			// Validate report data
			let runtime_info_hash = hashing::blake2_512(&encoded_runtime_info);
			ensure!(runtime_info_hash.to_vec() == report_data, Error::<T>::InvalidRuntimeInfoHash);
			let runtime_info = PRuntimeInfo::decode(&mut &encoded_runtime_info[..]).map_err(|_| Error::<T>::InvalidRuntimeInfo)?;
			let machine_id = runtime_info.machine_id.to_vec();
			// Add into the registry
			// TODO: Now we just force remove the worker and thus stop the mining. Should we just
			// update the worker info if there's an existing one?
			let perv_worker_info = Self::remove_machine_if_present(&machine_id);
			let last_updated = T::UnixTime::now().as_millis().saturated_into::<u64>();
			let pubkey = runtime_info.pubkey.to_vec();
			let score = Some(Score {
				overall_score: calc_overall_score(&runtime_info.features).map_err(|()| Error::<T>::InvalidInput)?,
				features: runtime_info.features
			});
			let worker_info = match perv_worker_info {
				Some(info) => WorkerInfo {
					pubkey,
					last_updated,
					score,
					..info
				},
				None => WorkerInfo {
					machine_id: machine_id.clone(),
					pubkey,
					last_updated,
					score,
					status: 0,
				},
			};
			WorkerState::<T>::insert(&stash, worker_info);
			MachineOwner::<T>::insert(&machine_id, &stash);
			Self::deposit_event(RawEvent::WorkerRegistered(stash, machine_id));
			Ok(())
		}

		#[weight = 0]
		fn force_register_worker(origin, stash: T::AccountId, machine_id: Vec<u8>, pubkey: Vec<u8>) -> dispatch::DispatchResult {
			ensure_root(origin)?;
			ensure!(StashState::<T>::contains_key(&stash), Error::<T>::StashNotFound);
			Self::remove_machine_if_present(&machine_id);
			let worker_info = WorkerInfo {
				machine_id: machine_id.clone(),
				pubkey,
				last_updated: T::UnixTime::now().as_millis().saturated_into::<u64>(),
				status: 0,
				score: Some(Score {
					overall_score: 100,
					features: vec![1, 4]
				}),
			};
			WorkerState::<T>::insert(&stash, worker_info);
			MachineOwner::<T>::insert(&machine_id, &stash);
			Self::deposit_event(RawEvent::WorkerRegistered(stash, machine_id));
			Ok(())
		}

		#[weight = 0]
		fn force_set_contract_key(origin, id: u32, pubkey: Vec<u8>) -> dispatch::DispatchResult {
			ensure_root(origin)?;
			ContractKey::insert(id, pubkey);
			Ok(())
		}

		// Mining

		#[weight = 0]
		fn start_mine(origin) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Stash::<T>::contains_key(&who), Error::<T>::ControllerNotFound);
			let stash = Stash::<T>::get(who);
			WorkerState::<T>::mutate(&stash, |worker_info| worker_info.status = 1);
			Ok(())
		}

		#[weight = 0]
		fn stop_mine(origin) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Stash::<T>::contains_key(&who), Error::<T>::ControllerNotFound);
			let stash = Stash::<T>::get(who);
			WorkerState::<T>::mutate(&stash, |worker_info| worker_info.status = 0);
			Ok(())
		}

		#[weight = 0]
		fn claim_reward(origin, stash: T::AccountId) -> dispatch::DispatchResult {
			ensure_signed(origin)?;
			// invoked by anyone
			Ok(())
		}

		// Token

		#[weight = 0]
		fn transfer_to_tee(origin, #[compact] amount: BalanceOf<T>) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			T::TEECurrency::transfer(&who, &Self::account_id(), amount, AllowDeath)
				.map_err(|_| Error::<T>::CannotDeposit)?;
			Self::deposit_event(RawEvent::TransferToTee(who.encode(), amount));
			Ok(())
		}

		#[weight = 0]
		fn transfer_to_chain(origin, data: Vec<u8>) -> dispatch::DispatchResult {
			// This is a specialized Contract-to-Chain message passing where the confidential
			// contract is always Balances (id = 2)
			// Anyone can call this method. As long as the message meets all the requirements
			// (signature, sequence id, etc), it's considered as a valid message.
			const CONTRACT_ID: u32 = 2;
			ensure_signed(origin)?;
			let transfer_data: TransferData<<T as frame_system::Trait>::AccountId, BalanceOf<T>>
				= Decode::decode(&mut &data[..]).map_err(|_| Error::<T>::InvalidInput)?;
			// Check sequence
			let sequence = IngressSequence::get(CONTRACT_ID);
			ensure!(transfer_data.data.sequence == sequence + 1, Error::<T>::BadMessageSequence);
			// Contract key
			ensure!(ContractKey::contains_key(CONTRACT_ID), Error::<T>::InvalidContract);
			let pubkey = ContractKey::get(CONTRACT_ID);
			// Validate TEE signature
			Self::verify_signature(&pubkey, &transfer_data)?;
			// Release funds
			T::TEECurrency::transfer(
				&Self::account_id(), &transfer_data.data.dest, transfer_data.data.amount,
				AllowDeath)
				.map_err(|_| Error::<T>::CannotWithdraw)?;
			// Announce the successful execution
			IngressSequence::insert(CONTRACT_ID, sequence + 1);
			Self::deposit_event(RawEvent::TransferToChain(transfer_data.data.dest.encode(), transfer_data.data.amount, sequence + 1));
			Ok(())
		}

		#[weight = 0]
		fn heartbeat(origin, data: Vec<u8>) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			// Decode payload
			let heartbeat_data: HeartbeatData = Decode::decode(&mut &data[..]).map_err(|_| Error::<T>::InvalidInput)?;
			// Get identity key from controller
			ensure!(Stash::<T>::contains_key(&who), Error::<T>::ControllerNotFound);
			let stash = Stash::<T>::get(&who);
			let worker_info = WorkerState::<T>::get(&stash);
			// Validate TEE signature
			Self::verify_signature(&worker_info.pubkey, &heartbeat_data)?;
			// Emit event
			Self::deposit_event(RawEvent::Heartbeat(stash, heartbeat_data.data.block_num));
			Ok(())
		}

		// Borrowing
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		PALLET_ID.into_account()
	}

	pub fn is_controller(controller: T::AccountId) -> bool {
		Stash::<T>::contains_key(&controller)
	}
	pub fn verify_signature(serialized_pk: &Vec<u8>, data: &impl SignedDataType<Vec<u8>>) -> dispatch::DispatchResult {
		let pub_key = Self::try_parse_ecdsa_key(serialized_pk)?;
		let signature = secp256k1::Signature::parse_slice(&data.signature())
			.map_err(|_| Error::<T>::InvalidSignature)?;

		let msg_hash = hashing::blake2_256(&data.raw_data());
		let mut buffer = [0u8; 32];
		buffer.copy_from_slice(&msg_hash);
		let message = secp256k1::Message::parse(&buffer);

		let verified = secp256k1::verify(&message, &signature, &pub_key);
		ensure!(verified, Error::<T>::FailedToVerify);

		Ok(())
	}

	/// Try to remove a registered worker from the registry by its `machine_id` identity if
	/// presents, keeping the stash untouched
	fn remove_machine_if_present(machine_id: &Vec<u8>) -> Option<WorkerInfo> {
		if !MachineOwner::<T>::contains_key(machine_id) {
			return None;
		}
		let stash = MachineOwner::<T>::take(machine_id);
		let worker_info = WorkerState::<T>::take(&stash);
		Self::deposit_event(RawEvent::WorkerUnregistered(stash, machine_id.clone()));
		Some(worker_info)
	}

	fn try_parse_ecdsa_key(serialized_pk: &Vec<u8>) -> Result<secp256k1::PublicKey, Error<T>> {
		let mut pk = [0u8; 33];
		if serialized_pk.len() != 33 {
			return Err(Error::<T>::InvalidPubKey);
		}
		pk.copy_from_slice(&serialized_pk);
		secp256k1::PublicKey::parse_compressed(&pk)
			.map_err(|_| Error::<T>::InvalidPubKey)
	}
}

fn calc_overall_score(features: &Vec<u32>) -> Result<u32, ()> {
	if features.len() != 2 {
		return Err(())
	}
	let core = features[0];
	let feature_level = features[1];
	Ok(core * (feature_level * 10 + 60))
}
