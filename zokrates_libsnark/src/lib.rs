mod ffi;
pub mod gm17;
pub mod pghr13;

use std::cmp::max;
use std::collections::HashMap;
use zokrates_ast::common::Variable;
use zokrates_ast::ir::{self, Statement};
use zokrates_field::Field;

pub struct Libsnark;

// utility function. Converts a Field's vector-based byte representation to fixed size array.
fn vec_as_u8_32_array(vec: &[u8]) -> [u8; 32] {
    assert!(vec.len() <= 32);
    let mut array = [0u8; 32];
    for (index, byte) in vec.iter().enumerate() {
        array[31 - index] = *byte;
    }
    array
}

pub fn prepare_public_inputs<T: Field>(public_inputs: Vec<T>) -> (Vec<[u8; 32]>, usize) {
    let public_inputs_length = public_inputs.len();
    let mut public_inputs_arr: Vec<[u8; 32]> = vec![[0u8; 32]; public_inputs_length];

    for (index, value) in public_inputs.into_iter().enumerate() {
        public_inputs_arr[index] = vec_as_u8_32_array(&value.to_byte_vector());
    }

    (public_inputs_arr, public_inputs_length)
}

// proof-system-independent preparation for the setup phase
#[allow(clippy::type_complexity)]
pub fn prepare_setup<T: Field>(
    program: ir::Prog<T>,
) -> (
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<(i32, i32, [u8; 32])>,
    Vec<(i32, i32, [u8; 32])>,
    Vec<(i32, i32, [u8; 32])>,
    usize,
    usize,
    usize,
) {
    // transform to R1CS
    let (variables, public_variables_count, constraints) = r1cs_program(program);

    let num_inputs = public_variables_count - 1;
    let num_constraints = constraints.len();

    let num_variables = variables.len();

    // Create single A,B,C vectors of tuples (constraint_number, variable_id, variable_value)
    let mut a_vec = vec![];
    let mut b_vec = vec![];
    let mut c_vec = vec![];
    for (row, (a, b, c)) in constraints.iter().enumerate() {
        for &(idx, ref val) in a {
            a_vec.push((
                row as i32,
                idx as i32,
                vec_as_u8_32_array(&val.to_byte_vector()),
            ));
        }
        for &(idx, ref val) in b {
            b_vec.push((
                row as i32,
                idx as i32,
                vec_as_u8_32_array(&val.to_byte_vector()),
            ));
        }
        for &(idx, ref val) in c {
            c_vec.push((
                row as i32,
                idx as i32,
                vec_as_u8_32_array(&val.to_byte_vector()),
            ));
        }
    }

    // Sizes and offsets in bytes for our struct {row, id, value}
    // We're building { i32, i32, i8[32] }
    const STRUCT_SIZE: usize = 40;

    const ROW_SIZE: usize = 4;

    const IDX_SIZE: usize = 4;
    const IDX_OFFSET: usize = 4;

    const VALUE_SIZE: usize = 32;
    const VALUE_OFFSET: usize = 8;

    // Convert above A,B,C vectors to byte arrays for cpp
    let mut a_arr: Vec<u8> = vec![0u8; STRUCT_SIZE * a_vec.len()];
    let mut b_arr: Vec<u8> = vec![0u8; STRUCT_SIZE * b_vec.len()];
    let mut c_arr: Vec<u8> = vec![0u8; STRUCT_SIZE * c_vec.len()];
    for (id, (row, idx, val)) in a_vec.iter().enumerate() {
        let row_bytes: [u8; ROW_SIZE] = row.to_le().to_ne_bytes();
        let idx_bytes: [u8; IDX_SIZE] = idx.to_le().to_ne_bytes();

        for x in 0..ROW_SIZE {
            a_arr[id * STRUCT_SIZE + x] = row_bytes[x];
        }
        for x in 0..IDX_SIZE {
            a_arr[id * STRUCT_SIZE + x + IDX_OFFSET] = idx_bytes[x];
        }
        for x in 0..VALUE_SIZE {
            a_arr[id * STRUCT_SIZE + x + VALUE_OFFSET] = val[x];
        }
    }
    for (id, (row, idx, val)) in b_vec.iter().enumerate() {
        let row_bytes: [u8; ROW_SIZE] = row.to_le().to_ne_bytes();
        let idx_bytes: [u8; IDX_SIZE] = idx.to_le().to_ne_bytes();

        for x in 0..ROW_SIZE {
            b_arr[id * STRUCT_SIZE + x] = row_bytes[x];
        }
        for x in 0..IDX_SIZE {
            b_arr[id * STRUCT_SIZE + x + IDX_OFFSET] = idx_bytes[x];
        }
        for x in 0..VALUE_SIZE {
            b_arr[id * STRUCT_SIZE + x + VALUE_OFFSET] = val[x];
        }
    }
    for (id, (row, idx, val)) in c_vec.iter().enumerate() {
        let row_bytes: [u8; ROW_SIZE] = row.to_le().to_ne_bytes();
        let idx_bytes: [u8; IDX_SIZE] = idx.to_le().to_ne_bytes();

        for x in 0..ROW_SIZE {
            c_arr[id * STRUCT_SIZE + x] = row_bytes[x];
        }
        for x in 0..IDX_SIZE {
            c_arr[id * STRUCT_SIZE + x + IDX_OFFSET] = idx_bytes[x];
        }
        for x in 0..VALUE_SIZE {
            c_arr[id * STRUCT_SIZE + x + VALUE_OFFSET] = val[x];
        }
    }

    (
        a_arr,
        b_arr,
        c_arr,
        a_vec,
        b_vec,
        c_vec,
        num_constraints,
        num_variables,
        num_inputs,
    )
}

// proof-system-independent preparation for proof generation
pub fn prepare_generate_proof<T: Field>(
    program: ir::Prog<T>,
    witness: ir::Witness<T>,
) -> (Vec<[u8; 32]>, usize, Vec<[u8; 32]>, usize) {
    // recover variable order from the program
    let (variables, public_variables_count, _) = r1cs_program(program);

    let witness: Vec<_> = variables.iter().map(|x| witness.0[x].clone()).collect();

    // split witness into public and private inputs at offset
    let mut public_inputs: Vec<_> = witness;
    let private_inputs: Vec<_> = public_inputs.split_off(public_variables_count);

    let public_inputs_length = public_inputs.len();
    let private_inputs_length = private_inputs.len();

    let mut public_inputs_arr: Vec<[u8; 32]> = vec![[0u8; 32]; public_inputs_length];
    // length must not be zero here, so we apply the max function
    let mut private_inputs_arr: Vec<[u8; 32]> = vec![[0u8; 32]; max(private_inputs_length, 1)];

    //convert inputs
    for (index, value) in public_inputs.into_iter().enumerate() {
        public_inputs_arr[index] = vec_as_u8_32_array(&value.to_byte_vector());
    }
    for (index, value) in private_inputs.into_iter().enumerate() {
        private_inputs_arr[index] = vec_as_u8_32_array(&value.to_byte_vector());
    }

    (
        public_inputs_arr,
        public_inputs_length,
        private_inputs_arr,
        private_inputs_length,
    )
}

/// Returns the index of `var` in `variables`, adding `var` with incremented index if it does not yet exists.
///
/// # Arguments
///
/// * `variables` - A mutual map that maps all existing variables to their index.
/// * `var` - Variable to be searched for.
pub fn provide_variable_idx(variables: &mut HashMap<Variable, usize>, var: &Variable) -> usize {
    let index = variables.len();
    *variables.entry(*var).or_insert(index)
}

type LinComb<T> = Vec<(usize, T)>;
type Constraint<T> = (LinComb<T>, LinComb<T>, LinComb<T>);

/// Calculates one R1CS row representation of a program and returns (V, A, B, C) so that:
/// * `V` contains all used variables and the index in the vector represents the used number in `A`, `B`, `C`
/// * `<A,x>*<B,x> = <C,x>` for a witness `x`
///
/// # Arguments
///
/// * `prog` - The program the representation is calculated for.
pub fn r1cs_program<T: Field>(prog: ir::Prog<T>) -> (Vec<Variable>, usize, Vec<Constraint<T>>) {
    let mut variables: HashMap<Variable, usize> = HashMap::new();
    provide_variable_idx(&mut variables, &Variable::one());

    for x in prog.arguments.iter().filter(|p| !p.private) {
        provide_variable_idx(&mut variables, &x.id);
    }

    // ~out are added after main's arguments, since we want variables (columns)
    // in the r1cs to be aligned like "public inputs | private inputs"
    let main_return_count = prog.returns().len();

    for i in 0..main_return_count {
        provide_variable_idx(&mut variables, &Variable::public(i));
    }

    // position where private part of witness starts
    let private_inputs_offset = variables.len();

    // first pass through statements to populate `variables`
    for (quad, lin) in prog.statements.iter().filter_map(|s| match s {
        Statement::Constraint(quad, lin, _) => Some((quad, lin)),
        Statement::Directive(..) => None,
    }) {
        for (k, _) in &quad.left.0 {
            provide_variable_idx(&mut variables, k);
        }
        for (k, _) in &quad.right.0 {
            provide_variable_idx(&mut variables, k);
        }
        for (k, _) in &lin.0 {
            provide_variable_idx(&mut variables, k);
        }
    }

    let mut constraints = vec![];

    // second pass to convert program to raw sparse vectors
    for (quad, lin) in prog.statements.into_iter().filter_map(|s| match s {
        Statement::Constraint(quad, lin, _) => Some((quad, lin)),
        Statement::Directive(..) => None,
    }) {
        constraints.push((
            quad.left
                .0
                .into_iter()
                .map(|(k, v)| (*variables.get(&k).unwrap(), v))
                .collect(),
            quad.right
                .0
                .into_iter()
                .map(|(k, v)| (*variables.get(&k).unwrap(), v))
                .collect(),
            lin.0
                .into_iter()
                .map(|(k, v)| (*variables.get(&k).unwrap(), v))
                .collect(),
        ));
    }

    // Convert map back into list ordered by index
    let mut variables_list = vec![Variable::new(0); variables.len()];
    for (k, v) in variables.drain() {
        assert_eq!(variables_list[v], Variable::new(0));
        variables_list[v] = k;
    }
    (variables_list, private_inputs_offset, constraints)
}

pub mod serialization {
    use std::io::Error;
    use std::io::Read;
    use std::io::Write;
    use zokrates_proof_systems::{G1Affine, G2Affine, G2AffineFq2};

    #[inline]
    fn decode_hex(value: &str) -> Vec<u8> {
        hex::decode(value.strip_prefix("0x").unwrap()).unwrap()
    }

    #[inline]
    fn encode_hex<T: AsRef<[u8]>>(data: T) -> String {
        format!("0x{}", hex::encode(data))
    }

    pub fn read_g1<R: Read>(reader: &mut R) -> Result<G1Affine, Error> {
        let mut buffer = [0; 64];
        reader.read_exact(&mut buffer)?;

        Ok(G1Affine(
            encode_hex(&buffer[0..32]),
            encode_hex(&buffer[32..64]),
        ))
    }

    pub fn read_g2<R: Read>(reader: &mut R) -> Result<G2Affine, Error> {
        let mut buffer = [0; 128];
        reader.read_exact(&mut buffer)?;

        Ok(G2Affine::Fq2(G2AffineFq2(
            (encode_hex(&buffer[0..32]), encode_hex(&buffer[32..64])),
            (encode_hex(&buffer[64..96]), encode_hex(&buffer[96..128])),
        )))
    }

    pub fn write_g1<W: Write>(writer: &mut W, g1: &G1Affine) {
        writer.write_all(decode_hex(&g1.0).as_ref()).unwrap();
        writer.write_all(decode_hex(&g1.1).as_ref()).unwrap();
    }

    pub fn write_g2<W: Write>(writer: &mut W, g2: &G2Affine) {
        match g2 {
            G2Affine::Fq2(g2) => {
                writer.write_all(decode_hex(&(g2.0).0).as_ref()).unwrap();
                writer.write_all(decode_hex(&(g2.0).1).as_ref()).unwrap();
                writer.write_all(decode_hex(&(g2.1).0).as_ref()).unwrap();
                writer.write_all(decode_hex(&(g2.1).1).as_ref()).unwrap();
            }
            _ => unreachable!(),
        }
    }
}
