use halo2_base::{
    halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::{
            bn256::{Fq, Fq2, Fr, G1Affine},
            FieldExt,
        },
        plonk::{Circuit, ConstraintSystem, Error},
    },
    Context, ContextParams,
};
use halo2_ecc::{
    bn254::{pairing::PairingChip, Fp2Chip, FpChip},
    fields::{fp::FpStrategy, FieldChip},
};
use num_bigint::BigUint;
use num_traits::Num;

const K: usize = 16;

const NUM_ADVICE: usize = 160;
const NUM_LOOKUP_ADVICE: usize = 6;
const LIMB_BITS: usize = 90;
const NUM_LIMBS: usize = 3;
const LOOKUP_BITS: usize = 15;
const NUM_FIXED: usize = 1;

use crate::map_to_curve::map_to_curve;

#[derive(Clone, Default)]
struct MyCircuit;

impl Circuit<Fr> for MyCircuit {
    type Config = FpChip<Fr>;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        let p = BigUint::from_str_radix(&Fq::MODULUS[2..], 16).unwrap();
        FpChip::configure(
            meta,
            FpStrategy::Simple,
            &[NUM_ADVICE],
            &[NUM_LOOKUP_ADVICE],
            NUM_FIXED,
            LOOKUP_BITS,
            LIMB_BITS,
            NUM_LIMBS,
            p,
            0,
            K,
        )
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        let fp_chip = config;
        let flex_chip = fp_chip.gate();
        fp_chip.load_lookup_table(&mut layouter)?;
        let fp2_chip = Fp2Chip::construct(&fp_chip);
        let pairing_chip = PairingChip::construct(&fp_chip);
        let mut rng = rand::thread_rng();

        layouter.assign_region(
            || "my region",
            |region| {
                let ctx = &mut Context::new(
                    region,
                    ContextParams {
                        max_rows: fp_chip.range.gate.max_rows,
                        num_context_ids: 2,
                        fixed_columns: fp_chip.range.gate.constants.clone(),
                    },
                );

                let u = Fq2 {
                    c0: Fq::from(5),
                    c1: Fq::from(7),
                };
                let u = fp2_chip.load_constant(ctx, u);

                let q_g2 = map_to_curve(ctx, &flex_chip, &fp_chip, &fp2_chip, &u);

                let p_g1 = G1Affine::random(&mut rng);
                let p_g1 = pairing_chip.load_private_g1(ctx, Value::known(p_g1));
                let fq_point = pairing_chip.pairing(ctx, &q_g2, &p_g1);
                let fq_point = pairing_chip.pairing(ctx, &q_g2, &p_g1);
                let fq_point = pairing_chip.pairing(ctx, &q_g2, &p_g1);

                Ok(())
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // use crate::utils::{gen_pk, gen_proof, gen_srs};
    // use std::time::Instant;

    use halo2_base::halo2_proofs::dev::MockProver;

    use super::*;

    #[test]
    fn test_bls_verify() {
        let circuit = MyCircuit;

        MockProver::run(K as u32, &circuit, vec![])
            .unwrap()
            .assert_satisfied();

        // let params = gen_srs(K as u32);
        // let pk = gen_pk(&params, &circuit);
        // let now = Instant::now();
        // gen_proof(K, &params, &pk, circuit, vec![]);
        // println!("{} ms", now.elapsed().as_millis());
    }
}
