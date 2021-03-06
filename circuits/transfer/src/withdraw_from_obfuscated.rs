// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::gadgets;

use anyhow::Result;
use dusk_pki::{PublicSpendKey, ViewKey};
use dusk_plonk::constraint_system::ecc::scalar_mul::variable_base::variable_base_scalar_mul;
use dusk_plonk::constraint_system::ecc::Point;
use dusk_plonk::jubjub::JubJubExtended;
use dusk_plonk::prelude::*;
use dusk_poseidon::cipher::{self, PoseidonCipher};
use phoenix_core::{Error as PhoenixError, Message, Note};

#[derive(Debug, Default, Clone)]
pub struct WithdrawFromObfuscatedCircuit {
    pi_positions: Vec<PublicInput>,

    input_value: u64,
    input_blinding_factor: JubJubScalar,
    change_r: JubJubScalar,
    change_value: u64,
    change_blinding_factor: JubJubScalar,
    output_value: u64,
    output_blinding_factor: JubJubScalar,

    // Public data
    input_value_commitment: JubJubExtended,
    change_value_commitment: JubJubExtended,
    change_nonce: JubJubScalar,
    change_cipher: [BlsScalar; PoseidonCipher::cipher_size()],
    change_pk: JubJubExtended,
    output_value_commitment: JubJubExtended,
}

impl From<&[PublicInput]> for WithdrawFromObfuscatedCircuit {
    // TODO
    // This should be removed after the `Circuit` trait of PLONK is refactored.
    //
    // Also, this implementation should be `TryFrom` - this is only a temporary
    // workaround to allow host verification. The invalid public points will
    // just be ignored.
    //
    // This implementation intentionally don't benefit from `Default` because
    // both need to be removed in the short term and its better if they are
    // completely decoupled
    fn from(pi: &[PublicInput]) -> Self {
        // TODO
        // The pi_positions is expected to be reconstructed inside the
        // `Circuit::gadget` function This will change as soon as
        // `Circuit` is refactored
        let pi_positions = vec![];

        let input_value = Default::default();
        let input_blinding_factor = Default::default();
        let change_r = Default::default();
        let change_value = Default::default();
        let change_blinding_factor = Default::default();
        let output_value = Default::default();
        let output_blinding_factor = Default::default();

        // Public data
        let mut input_value_commitment = Default::default();
        let mut change_value_commitment = Default::default();
        let mut change_nonce = Default::default();
        let mut change_pk = Default::default();
        let mut change_cipher: [BlsScalar; PoseidonCipher::cipher_size()] =
            Default::default();
        let mut output_value_commitment = Default::default();

        pi.iter().enumerate().for_each(|(i, p)| match (i, p) {
            (0, PublicInput::AffinePoint(p, _, _)) => {
                input_value_commitment = (*p).into()
            }

            (1, PublicInput::AffinePoint(p, _, _)) => {
                change_value_commitment = (*p).into()
            }

            (2, PublicInput::JubJubScalar(s, _)) => change_nonce = *s,

            (3, PublicInput::AffinePoint(p, _, _)) => change_pk = (*p).into(),

            (4, PublicInput::BlsScalar(s, _)) => change_cipher[0] = *s,

            (5, PublicInput::BlsScalar(s, _)) => change_cipher[1] = *s,

            (6, PublicInput::BlsScalar(s, _)) => change_cipher[2] = *s,

            (7, PublicInput::AffinePoint(p, _, _)) => {
                output_value_commitment = (*p).into()
            }

            _ => (),
        });

        Self {
            pi_positions,
            input_value,
            input_blinding_factor,
            change_r,
            change_value,
            change_blinding_factor,
            output_value,
            output_blinding_factor,
            input_value_commitment,
            change_value_commitment,
            change_nonce,
            change_cipher,
            change_pk,
            output_value_commitment,
        }
    }
}

impl WithdrawFromObfuscatedCircuit {
    pub const fn rusk_keys_id() -> &'static str {
        "transfer-withdraw-from-obfuscated"
    }

    pub fn new(
        input: &Note,
        input_view_key: Option<&ViewKey>,
        change: &Message,
        change_r: JubJubScalar,
        change_psk: &PublicSpendKey,
        output: &Note,
        output_view_key: Option<&ViewKey>,
    ) -> Result<Self, PhoenixError> {
        let input_value = input.value(input_view_key)?;
        let input_blinding_factor = input.blinding_factor(input_view_key)?;
        let input_value_commitment = *input.value_commitment();

        let change_pk = *change_psk.A();
        let change_value_commitment = *change.value_commitment();
        let change_nonce = *change.nonce();
        let change_cipher = *change.cipher();
        let (change_value, change_blinding_factor) =
            change.decrypt(&change_r, &change_psk)?;

        let output_value = output.value(output_view_key)?;
        let output_blinding_factor = output.blinding_factor(output_view_key)?;
        let output_value_commitment = *output.value_commitment();

        Ok(Self {
            pi_positions: vec![],
            input_value,
            input_blinding_factor,
            input_value_commitment,
            change_r,
            change_value,
            change_blinding_factor,
            change_value_commitment,
            change_nonce,
            change_cipher,
            change_pk,
            output_value,
            output_blinding_factor,
            output_value_commitment,
        })
    }
}

impl Circuit<'_> for WithdrawFromObfuscatedCircuit {
    fn gadget(&mut self, composer: &mut StandardComposer) -> Result<()> {
        let mut pi = vec![];

        // 1. Prove the knowledge of the commitment opening of the commitment of
        // the input
        let input_value = composer.add_input(self.input_value.into());

        let input_blinding_factor = self.input_blinding_factor.into();
        let input_blinding_factor = composer.add_input(input_blinding_factor);

        let input_value_commitment_p =
            gadgets::commitment(composer, input_value, input_blinding_factor);

        let input_value_commitment = self.input_value_commitment.into();
        pi.push(PublicInput::AffinePoint(
            input_value_commitment,
            composer.circuit_size(),
            composer.circuit_size() + 1,
        ));
        composer.assert_equal_public_point(
            input_value_commitment_p,
            input_value_commitment,
        );

        // 2. Prove that the value of the opening of the commitment of the input
        // is within range
        composer.range_gate(input_value, 64);

        // 3. Prove the knowledge of the commitment opening of the commitment of
        // the change
        let change_value = composer.add_input(self.change_value.into());

        let change_blinding_factor = self.change_blinding_factor.into();
        let change_blinding_factor = composer.add_input(change_blinding_factor);

        let change_value_commitment_p =
            gadgets::commitment(composer, change_value, change_blinding_factor);

        let change_value_commitment = self.change_value_commitment.into();
        pi.push(PublicInput::AffinePoint(
            change_value_commitment,
            composer.circuit_size(),
            composer.circuit_size() + 1,
        ));
        composer.assert_equal_public_point(
            change_value_commitment_p,
            change_value_commitment,
        );

        // 4. Message consistency

        // 5. Prove that the value of the opening of the commitment of the
        // change Message is within range
        composer.range_gate(change_value, 64);

        // 6. Prove that the encrypted value of the opening of the commitment of
        // the Message  is within correctly encrypted to the derivative of pk
        // 7. Prove that the encrypted blinder of the opening of the commitment
        // of the Message  is within correctly encrypted to the derivative of pk
        let change_nonce = self.change_nonce.into();
        let change_nonce_p = composer.add_input(change_nonce);
        pi.push(PublicInput::BlsScalar(
            change_nonce,
            composer.circuit_size(),
        ));
        composer.constrain_to_constant(
            change_nonce_p,
            BlsScalar::zero(),
            -change_nonce,
        );
        let change_nonce = change_nonce_p;

        let change_r = composer.add_input(self.change_r.into());
        let change_pk = self.change_pk.into();
        pi.push(PublicInput::AffinePoint(
            change_pk,
            composer.circuit_size(),
            composer.circuit_size() + 1,
        ));
        let change_pk = Point::from_public_affine(composer, change_pk);
        let cipher_secret =
            variable_base_scalar_mul(composer, change_r, change_pk);

        let change_cipher = cipher::encrypt(
            composer,
            cipher_secret.point(),
            change_nonce,
            &[change_value, change_blinding_factor],
        );

        self.change_cipher
            .iter()
            .zip(change_cipher.iter())
            .for_each(|(c, w)| {
                let c = *c;

                pi.push(PublicInput::BlsScalar(c, composer.circuit_size()));
                composer.constrain_to_constant(*w, BlsScalar::zero(), -c);
            });

        // 8. Prove the knowledge of the commitment opening of the commitment of
        // the output obfuscated note
        let output_value = composer.add_input(self.output_value.into());

        let output_blinding_factor = self.output_blinding_factor.into();
        let output_blinding_factor = composer.add_input(output_blinding_factor);

        let output_value_commitment_p =
            gadgets::commitment(composer, output_value, output_blinding_factor);

        let output_value_commitment = self.output_value_commitment.into();
        pi.push(PublicInput::AffinePoint(
            output_value_commitment,
            composer.circuit_size(),
            composer.circuit_size() + 1,
        ));
        composer.assert_equal_public_point(
            output_value_commitment_p,
            output_value_commitment,
        );

        // 9. Prove that the value of the opening of the commitment of the
        // output obfuscated note is within range
        composer.range_gate(output_value, 64);

        // 10. Prove that v_i - v_c - v_o = 0
        composer.poly_gate(
            input_value,
            change_value,
            output_value,
            BlsScalar::zero(),
            BlsScalar::one(),
            -BlsScalar::one(),
            -BlsScalar::one(),
            BlsScalar::zero(),
            BlsScalar::zero(),
        );

        self.get_mut_pi_positions().extend_from_slice(pi.as_slice());

        Ok(())
    }

    fn get_trim_size(&self) -> usize {
        1 << 13
    }

    fn set_trim_size(&mut self, _size: usize) {
        // N/A, fixed size circuit
    }

    fn get_mut_pi_positions(&mut self) -> &mut Vec<PublicInput> {
        &mut self.pi_positions
    }

    /// Return a reference to the Public Inputs storage of the circuit.
    fn get_pi_positions(&self) -> &Vec<PublicInput> {
        &self.pi_positions
    }
}

#[test]
fn withdraw_from_obfuscated() {
    use crate::test_helpers;

    use anyhow::anyhow;
    use dusk_pki::SecretSpendKey;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    let mut rng = StdRng::seed_from_u64(2324u64);

    test_helpers::circuit(
        &mut rng,
        WithdrawFromObfuscatedCircuit::rusk_keys_id(),
        |rng| {
            let i_ssk = SecretSpendKey::random(rng);
            let i_vk = i_ssk.view_key();
            let i_psk = i_ssk.public_spend_key();
            let i_value = 100;
            let i_blinding_factor = JubJubScalar::random(rng);
            let i_note =
                Note::obfuscated(rng, &i_psk, i_value, i_blinding_factor);

            let c_ssk = SecretSpendKey::random(rng);
            let c_psk = c_ssk.public_spend_key();
            let c_r = JubJubScalar::random(rng);
            let c_value = 25;
            let c = Message::new(rng, &c_r, &c_psk, c_value);

            let o_ssk = SecretSpendKey::random(rng);
            let o_vk = o_ssk.view_key();
            let o_psk = o_ssk.public_spend_key();
            let o_value = 75;
            let o_blinding_factor = JubJubScalar::random(rng);
            let o_note =
                Note::obfuscated(rng, &o_psk, o_value, o_blinding_factor);

            WithdrawFromObfuscatedCircuit::new(
                &i_note,
                Some(&i_vk),
                &c,
                c_r,
                &c_psk,
                &o_note,
                Some(&o_vk),
            )
            .map_err(|e| anyhow!("Error creating circuit: {:?}", e))
        },
    )
    .expect("Failed to build and execute circuit!");
}
