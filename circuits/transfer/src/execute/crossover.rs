// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use dusk_plonk::jubjub::JubJubExtended;
use dusk_plonk::prelude::*;

#[derive(Debug, Default, Clone)]
pub struct CircuitCrossover {
    value_commitment: JubJubExtended,
    value: u64,
    blinding_factor: JubJubScalar,
    fee: u64,
}

impl CircuitCrossover {
    pub fn new(
        value_commitment: JubJubExtended,
        value: u64,
        blinding_factor: JubJubScalar,
        fee: u64,
    ) -> Self {
        Self {
            value_commitment,
            value,
            blinding_factor,
            fee,
        }
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn value_commitment(&self) -> &JubJubExtended {
        &self.value_commitment
    }

    pub fn to_witness(
        &self,
        composer: &mut StandardComposer,
    ) -> WitnessCrossover {
        let value_commitment = self.value_commitment;

        let fee_value = BlsScalar::from(self.fee);
        let fee_value_witness = composer.add_input(fee_value);

        let value = BlsScalar::from(self.value);
        let value = composer.add_input(value);

        let blinding_factor = BlsScalar::from(self.blinding_factor);
        let blinding_factor = composer.add_input(blinding_factor);

        WitnessCrossover {
            value,
            fee_value_witness,
            blinding_factor,
            fee_value,
            value_commitment,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WitnessCrossover {
    pub value: Variable,
    pub fee_value_witness: Variable,
    pub blinding_factor: Variable,

    // Public data
    pub fee_value: BlsScalar,
    pub value_commitment: JubJubExtended,
}
