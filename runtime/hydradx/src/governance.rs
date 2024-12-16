// This file is part of HydraDX-node.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

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

use super::*;
use primitives::constants::{
	currency::{deposit, CENTS, DOLLARS, UNITS},
	time::{DAYS, HOURS},
};

use frame_support::{
	parameter_types,
	sp_runtime::{Perbill, Percent, Permill},
	traits::{
		fungible::HoldConsideration, tokens::UnityAssetBalanceConversion, ConstU32, EitherOfDiverse,
		LinearStoragePrice, LockIdentifier, PrivilegeCmp,
	},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::traits::IdentityLookup;
use sp_staking::currency_to_vote::U128CurrencyToVote;
use sp_std::cmp::Ordering;

parameter_types! {
	pub TreasuryAccount: AccountId = Treasury::account_id();
	pub const ProposalBond: Permill = Permill::from_percent(3);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 500 * DOLLARS;
	pub const SpendPeriod: BlockNumber = DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaxApprovals: u32 =  100;
	pub const TreasuryPayoutPeriod: u32 = 30 * DAYS;
}

pub struct PayFromTreasuryAccount;

impl frame_support::traits::tokens::Pay for PayFromTreasuryAccount {
	type Balance = Balance;
	type Beneficiary = AccountId;
	type AssetKind = ();
	type Id = ();
	type Error = sp_runtime::DispatchError;

	#[cfg(not(feature = "runtime-benchmarks"))]
	fn pay(
		who: &Self::Beneficiary,
		_asset_kind: Self::AssetKind,
		amount: Self::Balance,
	) -> Result<Self::Id, Self::Error> {
		let _ = <Balances as frame_support::traits::fungible::Mutate<_>>::transfer(
			&TreasuryAccount::get(),
			who,
			amount,
			frame_support::traits::tokens::Preservation::Expendable,
		)?;
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn pay(
		who: &Self::Beneficiary,
		_asset_kind: Self::AssetKind,
		amount: Self::Balance,
	) -> Result<Self::Id, Self::Error> {
		// In case of benchmarks, we adjust the value by multiplying it by 1_000_000_000_000, otherwise it fails with BelowMinimum limit error, because
		// treasury benchmarks uses only 100 as the amount.
		let _ = <Balances as frame_support::traits::fungible::Mutate<_>>::transfer(
			&TreasuryAccount::get(),
			who,
			amount * 1_000_000_000_000,
			frame_support::traits::tokens::Preservation::Expendable,
		)?;
		Ok(())
	}

	fn check_payment(_id: Self::Id) -> frame_support::traits::tokens::PaymentStatus {
		frame_support::traits::tokens::PaymentStatus::Success
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn ensure_successful(_: &Self::Beneficiary, _: Self::AssetKind, amount: Self::Balance) {
		<Balances as frame_support::traits::fungible::Mutate<_>>::mint_into(
			&TreasuryAccount::get(),
			amount * 1_000_000_000_000,
		)
		.unwrap();
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn ensure_concluded(_: Self::Id) {}
}

impl pallet_treasury::Config for Runtime {
	type Currency = Balances;
	type RejectOrigin = MoreThanHalfCouncil;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = TreasuryPalletId;
	type BurnDestination = ();
	type WeightInfo = weights::pallet_treasury::HydraWeight<Runtime>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<Balance>;
	#[cfg(feature = "runtime-benchmarks")]
	type SpendOrigin =
		frame_system::EnsureWithSuccess<EnsureRoot<AccountId>, AccountId, crate::benches::BenchmarkMaxBalance>;
	type AssetKind = (); // set to () to support only the native currency
	type Beneficiary = AccountId;
	type BeneficiaryLookup = IdentityLookup<AccountId>;
	type Paymaster = PayFromTreasuryAccount;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = TreasuryPayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = (); // default impl is enough because we support only the native currency
}

parameter_types! {
	pub PreimageBaseDeposit: Balance = deposit(2, 64);
	pub PreimageByteDeposit: Balance = deposit(0, 1);
	pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_preimage::HydraWeight<Runtime>;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type Consideration = HoldConsideration<
		AccountId,
		Balances,
		PreimageHoldReason,
		LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
	>;
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
	fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
		if left == right {
			return Some(Ordering::Equal);
		}

		match (left, right) {
			// Root is greater than anything.
			(OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
			// Check which one has more yes votes.
			(
				OriginCaller::Council(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
				OriginCaller::Council(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
			) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
			// For every other origin we don't care, as they are not used for `ScheduleOrigin`.
			_ => None,
		}
	}
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
}
impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = MoreThanHalfCouncil;
	type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = weights::pallet_scheduler::HydraWeight<Runtime>;
	type Preimages = Preimage;
}

parameter_types! {
	pub const CouncilMaxProposals: u32 = 30;
	pub const CouncilMaxMembers: u32 = 13;
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub MaxProposalWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
}

pub type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective_council::HydraWeight<Runtime>;
	type MaxProposalWeight = MaxProposalWeight;
	type SetMembersOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	pub const TechnicalMaxProposals: u32 = 20;
	pub const TechnicalMaxMembers: u32 = 10;
	pub const TechnicalMotionDuration: BlockNumber = 5 * DAYS;
}

pub type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective_technical_committee::HydraWeight<Runtime>;
	type MaxProposalWeight = MaxProposalWeight;
	type SetMembersOrigin = EnsureRoot<AccountId>;
}

#[cfg(test)]
mod tests {
	use super::{EnactmentPeriod, VoteLockingPeriod};

	#[test]
	fn democracy_periods() {
		// Make sure VoteLockingPeriod > EnactmentPeriod
		assert!(VoteLockingPeriod::get() > EnactmentPeriod::get());
	}
}

pub type TreasuryApproveOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
>;

pub type MoreThanHalfCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

pub type MajorityOfCouncil = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
	EnsureRoot<AccountId>,
>;

pub type AllCouncilMembers = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
	EnsureRoot<AccountId>,
>;

pub type MoreThanHalfTechCommittee = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 2>,
	EnsureRoot<AccountId>,
>;

pub type SuperMajorityTechCommittee = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
	EnsureRoot<AccountId>,
>;

pub type AllTechnicalCommitteeMembers = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
	EnsureRoot<AccountId>,
>;

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 3 * DAYS;
	pub const VotingPeriod: BlockNumber = 3 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
	pub const MinimumDeposit: Balance = 1000 * DOLLARS;
	pub const EnactmentPeriod: BlockNumber = 24 * HOURS;
	// Make sure VoteLockingPeriod > EnactmentPeriod
	pub const VoteLockingPeriod: BlockNumber = 6 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	pub const InstantAllowed: bool = true;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type WeightInfo = weights::pallet_democracy::HydraWeight<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Scheduler = Scheduler;
	type Preimages = Preimage;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MinimumDeposit = MinimumDeposit;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	type CooloffPeriod = CooloffPeriod;
	type MaxVotes = MaxVotes;
	type MaxProposals = MaxProposals;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = MoreThanHalfCouncil;
	type ExternalMajorityOrigin = MoreThanHalfCouncil;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = AllCouncilMembers;
	type SubmitOrigin = EnsureSigned<AccountId>;
	type FastTrackOrigin = MoreThanHalfTechCommittee;
	type InstantOrigin = AllTechnicalCommitteeMembers;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = MajorityOfCouncil;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = AllTechnicalCommitteeMembers;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type PalletsOrigin = OriginCaller;
	type Slash = Treasury;
	type DemocracyHooks = pallet_staking::integrations::democracy::StakingDemocracy<Runtime>;
	// Any single technical committee member may remove a vote.
	type VoteRemovalOrigin = frame_system::EnsureSignedBy<TechCommAccounts, AccountId>;
}

parameter_types! {
	// Bond for candidacy into governance
	pub const CandidacyBond: Balance = 5 * DOLLARS;
	// 1 storage item created, key size is 32 bytes, value size is 16+16.
	pub const VotingBondBase: Balance = CENTS;
	// additional data per vote is 32 bytes (account id).
	pub const VotingBondFactor: Balance = CENTS;
	pub const TermDuration: BlockNumber = 7 * DAYS;
	pub const DesiredMembers: u32 = 13;
	pub const DesiredRunnersUp: u32 = 15;
	pub const ElectionsPhragmenPalletId: LockIdentifier = *b"phrelect";
	pub const MaxElectionCandidates: u32 = 100;
	pub const MaxElectionVoters: u32 = 768;
	pub const MaxVotesPerVoter: u32 = 10;
}

impl pallet_elections_phragmen::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ElectionsPhragmenPalletId;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = ();
	// Set to () if defined in chain spec
	type CurrencyToVote = U128CurrencyToVote;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = VotingBondBase;
	type VotingBondFactor = VotingBondFactor;
	type LoserCandidate = Treasury;
	type KickedMember = Treasury;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type TermDuration = TermDuration;
	type MaxCandidates = MaxElectionCandidates;
	type MaxVoters = MaxElectionVoters;
	type WeightInfo = weights::pallet_elections_phragmen::HydraWeight<Runtime>;
	type MaxVotesPerVoter = MaxVotesPerVoter;
}

parameter_types! {
	pub const DataDepositPerByte: Balance = CENTS;
	pub const TipCountdown: BlockNumber = 24 * HOURS;
	pub const TipFindersFee: Percent = Percent::from_percent(1);
	pub const TipReportDepositBase: Balance = 10 * DOLLARS;
	pub const TipReportDepositPerByte: Balance = CENTS;
	pub const MaximumReasonLength: u32 = 1024;
	pub const MaxTipAmount: u128 = 5_000_000 * UNITS; // ~$100k
}

impl pallet_tips::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaximumReasonLength = MaximumReasonLength;
	type DataDepositPerByte = DataDepositPerByte;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type MaxTipAmount = MaxTipAmount;
	type Tippers = Elections;
	type OnSlash = Treasury;
	type WeightInfo = weights::pallet_tips::HydraWeight<Runtime>;
}
