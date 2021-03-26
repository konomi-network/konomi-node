use super::*;
use crate::mock::*;
use frame_support::sp_runtime::traits::Hash;
use frame_support::traits::OnFinalize;
use frame_support::{assert_noop, assert_ok};
use frame_system::InitKind;
use sp_runtime::{FixedU128, DispatchError, FixedPointNumber};

fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

#[test]
fn can_supply() {
	new_test_ext().execute_with(|| {

		assert_ok!(Lending::supply(
			Origin::signed(1),
			0,
			100000,
		));
    });
}

#[test]
fn can_borrow() {
	new_test_ext().execute_with(|| {

		assert_ok!(Lending::supply(
			Origin::signed(1),
			0,
			100000,
		));

        assert_ok!(Lending::supply(
			Origin::signed(2),
			1,
			100000,
		));

        assert_ok!(Lending::borrow(
			Origin::signed(2),
			0,
			10000,
		));

    });
}

#[test]
fn can_repay() {
	new_test_ext().execute_with(|| {

		assert_ok!(Lending::supply(
			Origin::signed(1),
			0,
			100000,
		));

        assert_ok!(Lending::supply(
			Origin::signed(2),
			1,
			100000,
		));

        assert_ok!(Lending::borrow(
			Origin::signed(2),
			0,
			10000,
		));

		assert_ok!(Lending::repay(
			Origin::signed(2),
			0,
			10000,
		));

    });
}

#[test]
fn can_withdraw() {
	new_test_ext().execute_with(|| {

		assert_ok!(Lending::supply(
			Origin::signed(1),
			0,
			100000,
		));

        assert_ok!(Lending::supply(
			Origin::signed(2),
			1,
			100000,
		));

        assert_ok!(Lending::borrow(
			Origin::signed(2),
			0,
			10000,
		));

		assert_ok!(Lending::withdraw(
			Origin::signed(1),
			0,
			50000,
		));

    });
}

#[test]
fn can_liquidate() {
	new_test_ext().execute_with(|| {

		assert_ok!(Lending::supply(
			Origin::signed(1),
			0,
			100000,
		));

        assert_ok!(Lending::supply(
			Origin::signed(2),
			1,
			100000,
		));

        assert_ok!(Lending::borrow(
			Origin::signed(2),
			0,
			10000,
		));

		assert_ok!(Assets::set_price(
			Origin::root(),
			1,
			FixedU128::saturating_from_integer(5),
		));

        assert_ok!(Lending::liquidate(
			Origin::signed(1),
			2,
			0,
			1,
			10000,
		));


    });
}