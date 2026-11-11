use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    escrow_program::{constants::ESCROW_SEED, state::Escrow},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_rent::Rent,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_associated_token_account_interface::{
        address::get_associated_token_address, instruction::create_associated_token_account,
    },
    spl_token_interface::{
        instruction::{initialize_mint2, mint_to},
        state::{Account as TokenState, Mint},
    },
};

const DECIMALS: u8 = 6;
const START_BALANCE: u64 = 1_000;

/// A fresh world: two mints, a maker holding mint_a, and a taker holding mint_b.
struct World {
    svm: LiteSVM,
    program_id: Pubkey,
    maker: Keypair,
    taker: Keypair,
    mint_a: Pubkey,
    mint_b: Pubkey,
}

fn send(svm: &mut LiteSVM, payer: &Keypair, extra: &[&Keypair], ixs: &[Instruction]) -> Result<(), String> {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        &signers,
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e.err))
}

fn create_mint(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let mint = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(Mint::LEN);
    let create = solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        Mint::LEN as u64,
        &spl_token_interface::ID,
    );
    let init = initialize_mint2(
        &spl_token_interface::ID,
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        DECIMALS,
    )
    .unwrap();
    send(svm, payer, &[&mint], &[create, init]).unwrap();
    mint.pubkey()
}

fn token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm.get_account(token_account).expect("token account should exist");
    TokenState::unpack(&account.data).unwrap().amount
}

fn setup() -> World {
    let program_id = escrow_program::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrow_program.so"
    ));
    svm.add_program(program_id, bytes).unwrap();

    let maker = Keypair::new();
    let taker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&taker.pubkey(), 10_000_000_000).unwrap();

    let mint_a = create_mint(&mut svm, &maker);
    let mint_b = create_mint(&mut svm, &maker);

    // maker holds mint_a, taker holds mint_b
    let maker_ata_a = get_associated_token_address(&maker.pubkey(), &mint_a);
    let taker_ata_b = get_associated_token_address(&taker.pubkey(), &mint_b);
    let create_maker_ata = create_associated_token_account(
        &maker.pubkey(),
        &maker.pubkey(),
        &mint_a,
        &spl_token_interface::ID,
    );
    let create_taker_ata = create_associated_token_account(
        &maker.pubkey(),
        &taker.pubkey(),
        &mint_b,
        &spl_token_interface::ID,
    );
    let mint_to_maker = mint_to(
        &spl_token_interface::ID,
        &mint_a,
        &maker_ata_a,
        &maker.pubkey(),
        &[],
        START_BALANCE,
    )
    .unwrap();
    let mint_to_taker = mint_to(
        &spl_token_interface::ID,
        &mint_b,
        &taker_ata_b,
        &maker.pubkey(),
        &[],
        START_BALANCE,
    )
    .unwrap();
    send(
        &mut svm,
        &maker,
        &[],
        &[create_maker_ata, create_taker_ata, mint_to_maker, mint_to_taker],
    )
    .unwrap();

    World { svm, program_id, maker, taker, mint_a, mint_b }
}

fn escrow_pda(program_id: &Pubkey, maker: &Pubkey, seed: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[ESCROW_SEED, maker.as_ref(), &seed.to_le_bytes()],
        program_id,
    )
    .0
}

fn make_ix(w: &World, seed: u64, receive: u64, amount: u64) -> Instruction {
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);
    let vault = get_associated_token_address(&escrow, &w.mint_a);
    Instruction::new_with_bytes(
        w.program_id,
        &escrow_program::instruction::Make { seed, receive, amount }.data(),
        escrow_program::accounts::Make {
            maker: w.maker.pubkey(),
            mint_a: w.mint_a,
            mint_b: w.mint_b,
            maker_ata_a: get_associated_token_address(&w.maker.pubkey(), &w.mint_a),
            escrow,
            vault,
            associated_token_program: anchor_spl::associated_token::ID,
            token_program: spl_token_interface::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn take_ix(w: &World, seed: u64) -> Instruction {
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);
    let vault = get_associated_token_address(&escrow, &w.mint_a);
    Instruction::new_with_bytes(
        w.program_id,
        &escrow_program::instruction::Take {}.data(),
        escrow_program::accounts::Take {
            taker: w.taker.pubkey(),
            maker: w.maker.pubkey(),
            mint_a: w.mint_a,
            mint_b: w.mint_b,
            taker_ata_a: get_associated_token_address(&w.taker.pubkey(), &w.mint_a),
            taker_ata_b: get_associated_token_address(&w.taker.pubkey(), &w.mint_b),
            maker_ata_b: get_associated_token_address(&w.maker.pubkey(), &w.mint_b),
            escrow,
            vault,
            associated_token_program: anchor_spl::associated_token::ID,
            token_program: spl_token_interface::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn refund_ix(w: &World, seed: u64) -> Instruction {
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);
    let vault = get_associated_token_address(&escrow, &w.mint_a);
    Instruction::new_with_bytes(
        w.program_id,
        &escrow_program::instruction::Refund {}.data(),
        escrow_program::accounts::Refund {
            maker: w.maker.pubkey(),
            mint_a: w.mint_a,
            maker_ata_a: get_associated_token_address(&w.maker.pubkey(), &w.mint_a),
            escrow,
            vault,
            associated_token_program: anchor_spl::associated_token::ID,
            token_program: spl_token_interface::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn update_ix(w: &World, seed: u64, receive: u64) -> Instruction {
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);
    Instruction::new_with_bytes(
        w.program_id,
        &escrow_program::instruction::Update { receive }.data(),
        escrow_program::accounts::Update {
            maker: w.maker.pubkey(),
            escrow,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

#[test]
fn make_then_take_swaps_tokens_and_closes_accounts() {
    let mut w = setup();
    let seed = 1;
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);
    let vault = get_associated_token_address(&escrow, &w.mint_a);

    let make = make_ix(&w, seed, 200, 500);
    let maker = w.maker.insecure_clone();
    send(&mut w.svm, &maker, &[], &[make]).unwrap();
    assert_eq!(token_balance(&w.svm, &vault), 500);

    let take = take_ix(&w, seed);
    let taker = w.taker.insecure_clone();
    send(&mut w.svm, &taker, &[], &[take]).unwrap();

    let taker_ata_a = get_associated_token_address(&w.taker.pubkey(), &w.mint_a);
    let maker_ata_b = get_associated_token_address(&w.maker.pubkey(), &w.mint_b);
    assert_eq!(token_balance(&w.svm, &taker_ata_a), 500);
    assert_eq!(token_balance(&w.svm, &maker_ata_b), 200);
    assert!(w.svm.get_account(&escrow).is_none(), "escrow should be closed");
    assert!(w.svm.get_account(&vault).is_none(), "vault should be closed");
}

#[test]
fn make_then_refund_returns_tokens_to_maker() {
    let mut w = setup();
    let seed = 2;
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);
    let vault = get_associated_token_address(&escrow, &w.mint_a);
    let maker_ata_a = get_associated_token_address(&w.maker.pubkey(), &w.mint_a);

    let maker = w.maker.insecure_clone();
    let make = make_ix(&w, seed, 200, 500);
    send(&mut w.svm, &maker, &[], &[make]).unwrap();
    assert_eq!(token_balance(&w.svm, &maker_ata_a), START_BALANCE - 500);

    let refund = refund_ix(&w, seed);
    send(&mut w.svm, &maker, &[], &[refund]).unwrap();
    assert_eq!(token_balance(&w.svm, &maker_ata_a), START_BALANCE);
    assert!(w.svm.get_account(&escrow).is_none(), "escrow should be closed");
    assert!(w.svm.get_account(&vault).is_none(), "vault should be closed");
}

#[test]
fn update_changes_the_requested_amount() {
    let mut w = setup();
    let seed = 3;
    let escrow = escrow_pda(&w.program_id, &w.maker.pubkey(), seed);

    let maker = w.maker.insecure_clone();
    let make = make_ix(&w, seed, 200, 500);
    send(&mut w.svm, &maker, &[], &[make]).unwrap();
    let update = update_ix(&w, seed, 300);
    send(&mut w.svm, &maker, &[], &[update]).unwrap();

    let data = w.svm.get_account(&escrow).unwrap().data;
    let state = Escrow::try_deserialize(&mut data.as_slice()).unwrap();
    assert_eq!(state.receive, 300);
    assert_eq!(state.maker, w.maker.pubkey());
}

#[test]
fn make_with_zero_amount_fails() {
    let mut w = setup();
    let maker = w.maker.insecure_clone();
    let zero = make_ix(&w, 4, 200, 0);
    let result = send(&mut w.svm, &maker, &[], &[zero]);
    assert!(result.is_err(), "a zero amount should be rejected");
}

#[test]
fn only_the_maker_can_refund() {
    let mut w = setup();
    let seed = 5;
    let maker = w.maker.insecure_clone();
    let make = make_ix(&w, seed, 200, 500);
    send(&mut w.svm, &maker, &[], &[make]).unwrap();

    // The taker tries to call refund for the maker's escrow.
    let mut ix = refund_ix(&w, seed);
    ix.accounts[0].pubkey = w.taker.pubkey();
    ix.accounts[0].is_signer = true;
    let taker = w.taker.insecure_clone();
    let result = send(&mut w.svm, &taker, &[], &[ix]);
    assert!(result.is_err(), "only the maker may refund");
}

#[allow(dead_code)]
fn rent_of_mint(svm: &LiteSVM) -> u64 {
    svm.get_sysvar::<Rent>().minimum_balance(Mint::LEN)
}
