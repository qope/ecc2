use halo2_base::halo2_proofs::{
    circuit::{Cell, Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        bn256::{Fq, Fr, G1Affine},
        FieldExt,
    },
    plonk::{Circuit, Column, ConstraintSystem, Error, Instance},
};

use halo2_ecc::{bn254::FpChip, ecc::EccChip, fields::fp::FpStrategy};
use num_bigint::BigUint;
use num_traits::Num;

const K: usize = 16;

#[derive(Clone)]
struct MyConfig {
    fp_chip: FpChip<Fr>,
    instance: Column<Instance>,
}

#[derive(Clone, Default)]
struct MyCircuit {
    points: Vec<Option<G1Affine>>,
}

impl Circuit<Fr> for MyCircuit {
    type Config = MyConfig;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        let p = BigUint::from_str_radix(&Fq::MODULUS[2..], 16).unwrap();
        let fp_chip = FpChip::configure(
            meta,
            FpStrategy::Simple,
            &[100],
            &[16],
            1,
            14,
            88,
            3,
            p,
            0,
            K,
        );

        let instance = meta.instance_column();
        meta.enable_equality(instance);
        Self::Config { fp_chip, instance }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        // let mut instances = vec![];
        let fp_chip = config.clone().fp_chip;
        fp_chip.load_lookup_table(&mut layouter)?;
        let g1_chip = EccChip::construct(fp_chip.clone());

        let mut pi: Vec<Cell> = vec![];
        layouter.assign_region(
            || "my region",
            |region| {
                let mut aux = config.fp_chip.new_context(region);
                let ctx = &mut aux;

                let points = self
                    .points
                    .iter()
                    .cloned()
                    .map(|pt| {
                        g1_chip.assign_point(ctx, pt.map(Value::known).unwrap_or(Value::unknown()))
                    })
                    .collect::<Vec<_>>();
                let acc = g1_chip.sum::<G1Affine>(ctx, points.iter());

                let x_limbs = acc.x.limbs();
                for i in 0..pi.len() {
                    pi.push(*x_limbs[i].cell());
                }
                let y_limbs = acc.y.limbs();
                for i in 0..pi.len() {
                    pi.push(*y_limbs[i].cell());
                }

                fp_chip.finalize(ctx);
                Ok(())
            },
        )?;

        let instance_col = config.instance;

        for i in 0..pi.len() {
            layouter.constrain_instance(pi[i], instance_col, i);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{gen_pk, gen_proof, gen_srs};

    use super::*;
    use halo2_base::{halo2_proofs::dev::MockProver, utils::decompose_biguint};

    #[test]
    fn test_ecc_circuit() {
        let mut rng = rand::thread_rng();
        let batch_size = 1000;
        let mut points = Vec::new();
        for _ in 0..batch_size {
            let new_pt = Some(G1Affine::random(&mut rng));
            points.push(new_pt);
        }

        let answer = points
            .iter()
            .fold(G1Affine::default(), |a, b| (a + b.unwrap()).into());

        let x_dec = decompose_biguint::<Fr>(&BigUint::from_bytes_le(&answer.x.to_bytes()), 3, 88);
        let y_dec = decompose_biguint::<Fr>(&BigUint::from_bytes_le(&answer.y.to_bytes()), 3, 88);

        let mut instance = vec![];
        for i in 0..x_dec.len() {
            instance.push(x_dec[i]);
        }
        for i in 0..y_dec.len() {
            instance.push(y_dec[i]);
        }

        let circuit = MyCircuit { points };

        MockProver::run(K as u32, &circuit, vec![instance.clone()])
            .unwrap()
            .assert_satisfied();

        let params = gen_srs(K as u32);
        let pk = gen_pk(&params, &circuit);
        gen_proof(K, &params, &pk, circuit, vec![instance]);
    }
}
