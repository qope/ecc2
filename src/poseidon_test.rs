use std::marker::PhantomData;

use halo2_base::{
    gates::{
        flex_gate::{FlexGateConfig, GateStrategy},
        GateInstructions,
    },
    halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::bn256::Fr,
        plonk::{
            Advice, Challenge, Circuit, Column, ConstraintSystem, Error, FirstPhase, Instance,
        },
    },
    utils::ScalarField,
    Context, ContextParams, QuantumCell,
};
use poseidon::PoseidonChip;

const T: usize = 3;
const RATE: usize = 2;
const R_F: usize = 8;
const R_P: usize = 57;

const K: usize = 14;

#[derive(Clone, Default)]
struct MyCircuit;

impl Circuit<Fr> for MyCircuit {
    type Config = FlexGateConfig<Fr>;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        let flex_gate = FlexGateConfig::configure(meta, GateStrategy::Vertical, &[1000], 1, 0, K);
        flex_gate
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "my region",
            |region| {
                let ctx = &mut Context::new(
                    region,
                    ContextParams {
                        max_rows: K,
                        num_context_ids: 1,
                        fixed_columns: config.constants.clone(),
                    },
                );
                let mut poseidon_chip = PoseidonChip::<Fr, T, RATE>::new(ctx, &config, R_F, R_P)?;
                let one = config.load_constant(ctx, Fr::one());
                let two = config.load_constant(ctx, Fr::from(2));

                poseidon_chip.update(&[one.clone(), two.clone()]);
                let hashed = poseidon_chip.squeeze(ctx, &config)?;
                dbg!(&hashed);

                poseidon_chip.clear();
                poseidon_chip.update(&[one, two]);
                let hashed2 = poseidon_chip.squeeze(ctx, &config)?;
                dbg!(&hashed2);

                Ok(())
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::halo2_proofs::dev::MockProver;

    #[test]
    fn test_poseidon() {
        let circuit = MyCircuit;
        MockProver::run(K as u32, &circuit, vec![])
            .unwrap()
            .assert_satisfied();
    }
}
