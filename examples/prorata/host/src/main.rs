// Copyright 2023 RISC Zero, Inc.
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

//! The prorata host is a command-line tool that can be used to compute
//! allocations and verify receipts.

use ciborium;
use clap::{Parser, Subcommand};
use methods::{PRORATA_GUEST_ELF, PRORATA_GUEST_ID};
use prorata_core::{AllocationQuery, AllocationQueryResult};
use risc0_zkvm::{
    serde::{from_slice, to_vec},
    Executor, ExecutorEnv,
};
use rust_decimal::Decimal;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compute an allocation for one recipient
    Allocate {
        /// Input file to use (CSV)
        #[arg(short, long)]
        input: String,

        /// Output file to use (binary receipt)
        #[arg(short, long)]
        output: String,

        /// Recipient to compute allocation for
        #[arg(short, long)]
        recipient: String,

        /// Amount of funds to allocate
        #[arg(short, long)]
        amount: Decimal,
    },
    /// Verify an allocation read from a receipt
    Verify {
        /// Input file to use (binary receipt)
        #[arg(short, long)]
        input: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Allocate {
            input,
            output,
            recipient,
            amount,
        } => allocate(input, output, recipient, amount),
        Commands::Verify { input } => verify(input),
    }
}

/// Compute an allocation for one recipient based on the ownership indicated in
/// the input CSV file. The core operation is performed inside the zkVM and the
/// resulting receipt is stored to disk.
fn allocate(input: &str, output: &str, recipient: &str, amount: &Decimal) {
    println!("Query: {}", recipient);
    let recipients_csv = std::fs::read(&input).expect("Failed to read input file");

    let env = ExecutorEnv::builder()
        .add_input(
            &to_vec(&AllocationQuery {
                amount: *amount,
                recipients_csv,
                target: recipient.to_owned(),
            })
            .unwrap(),
        )
        .build();

    let mut exec = Executor::from_elf(env, PRORATA_GUEST_ELF).unwrap();
    let session = exec.run().unwrap();
    let receipt = session.prove().unwrap(); // Proof generation

    // Verify receipt to confirm that it is correctly formed. Not strictly
    // necessary.
    receipt.verify(PRORATA_GUEST_ID).unwrap();

    // Store the receipt to disk in CBOR format. Any serde-compatible library
    // could be used here.
    let writer = std::fs::File::create(output).expect("Failed to create output file");
    ciborium::ser::into_writer(&receipt, writer).unwrap();
}

/// Verify an allocation read from a receipt on disk.
fn verify(input: &str) {
    let reader = std::fs::File::open(input).expect("Failed to open input file");
    let receipt: risc0_zkvm::SessionReceipt = ciborium::de::from_reader(reader).unwrap();

    // Proof verification below
    match receipt.verify(PRORATA_GUEST_ID) {
        Ok(_) => {
            println!("Receipt is valid");
            let result: AllocationQueryResult =
                from_slice(&receipt.journal).expect("Failed to deserialize result");
            print!("{}", result);
        }
        Err(e) => println!("Receipt is invalid: {}", e),
    }
}
