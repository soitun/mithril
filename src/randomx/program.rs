use super::m128::{m128};
use strum::Display;
use std::fmt;

const MAX_FLOAT_REG : i64 = 4;
const MAX_REG : i64 = 8;
const STORE_L3_CONDITION : u8 = 14;
const SCRATCHPAD_L3_MASK : i32 = 0x1ffff8;
const REG_NEEDS_DISPLACEMENT: Store = Store::R5;

#[allow(nonstandard_style)]
#[derive(Display)]
pub enum Opcode {
    NOP = 0,
    IADD_RS = 0x10,
    IADD_M = 0x17,
    ISUB_R = 0x27,
    ISUB_M = 0x2e,
    IMUL_R = 0x3e,
    IMUL_M = 0x42,
    IMULH_R = 0x46,
    IMULH_M = 0x47,
    ISMULH_R = 0x4b,
    ISMULH_M = 0x4c,
    IMUL_RCP = 0x54,
    INEG_R = 0x56,
    IXOR_R = 0x65,
    IXOR_M = 0x6a,
    IROR_R = 0x72,
    IROL_R = 0x74,
    ISWAP_R = 0x78,
    FSWAP_R = 0x7c,
    FADD_R = 0x8c,
    FADD_M = 0x91,
    FSUB_R = 0xa1,
    FSUB_M = 0xa6,
    FSCAL_R = 0xac,
    FMUL_R = 0xcc,
    FDIV_M = 0xd0,
    FSQRT_R = 0xd6,
    CBRANCH = 0xef,
    CFROUND = 0xf0,
    ISTORE = 0x100,
}

#[derive(Display, PartialEq)]
pub enum Store {
    NONE,
    //registers
    #[strum(serialize = "r0")]
    R0,
    #[strum(serialize = "r1")]
    R1,
    #[strum(serialize = "r2")]
    R2,
    #[strum(serialize = "r3")]
    R3,
    #[strum(serialize = "r4")]
    R4,
    #[strum(serialize = "r5")]
    R5,
    #[strum(serialize = "r6")]
    R6,
    #[strum(serialize = "r7")]
    R7,
    //FP registers:
    #[strum(serialize = "f0")]
    F0,
    #[strum(serialize = "f1")]
    F1,
    #[strum(serialize = "f2")]
    F2,
    #[strum(serialize = "f3")]
    F3,
    #[strum(serialize = "e0")]
    E0,
    #[strum(serialize = "e1")]
    E1,
    #[strum(serialize = "e2")]
    E2,
    #[strum(serialize = "e3")]
    E3,
    #[strum(serialize = "a0")]
    A0,
    #[strum(serialize = "a1")]
    A1,
    #[strum(serialize = "a2")]
    A2,
    #[strum(serialize = "a3")]
    A3,
    #[strum(serialize = "i")]
    Imm, //non-register based Lx access
    //Lx memory
    L1(Box<Store>),
    L2(Box<Store>),
    L3(Box<Store>),
}

#[derive(PartialEq)]
pub enum Mode {
    None,
    Cond(u8),
    Shft(u8),
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::None => write!(f, "NONE"),
            Mode::Cond(x) => write!(f, "COND {}", x),
            Mode::Shft(x) => write!(f, "SHFT {}", x),
        }
    }
}

pub struct Instr {
    op: Opcode,
    src: Store,
    dst: Store,
    imm: Option<i32>,
    unsigned_imm: bool,
    mode: Mode,
    effect: fn(&mut State)
}

fn new_instr(op: Opcode, dst: Store, src: Store, imm: i32, mode: Mode) -> Instr {
    if src == dst {
        return Instr{op, dst, src: Store::NONE, imm: Some(imm), unsigned_imm: false, mode, effect: nop};
    }
    Instr{op, dst, src, imm: None, unsigned_imm: false, mode, effect: nop}
}

fn new_imm_instr(op: Opcode, dst: Store, imm: i32, mode: Mode) -> Instr {
    Instr{op, dst, src: Store::NONE, imm: Some(imm), unsigned_imm: false, mode, effect: nop}
}
 
fn new_lcache_instr(op: Opcode, dst_reg: Store, src: i64, imm: i32, modi: u8) -> Instr {
    let src_reg = r_reg(src);
    if src_reg == dst_reg {
        return Instr{op, dst: dst_reg, src: Store::L3(Box::new(Store::Imm)), imm: Some(imm & SCRATCHPAD_L3_MASK), unsigned_imm: false, mode: Mode::None, effect: nop};
    }
    return Instr{op, dst: dst_reg, src: l12_cache(src, modi), imm: Some(imm), unsigned_imm: false, mode: Mode::None, effect: nop}

}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.op)?;
        match &self.dst {
            Store::NONE => {/* do nothing */},
            Store::L1(reg) => write_l_access(f, self, reg, "L1")?,
            Store::L2(reg) => write_l_access(f, self, reg, "L2")?,
            Store::L3(reg) => write_l_access(f, self, reg, "L3")?,
            _ => write!(f, " {}", self.dst)?,
        }
        match &self.src {
            Store::NONE => {/* do nothing */},
            Store::L1(reg) => { write!(f, ",")?; write_l_access(f, self, reg, "L1")? },
            Store::L2(reg) => { write!(f, ",")?; write_l_access(f, self, reg, "L2")? },
            Store::L3(reg) => { write!(f, ",")?; write_l_access(f, self, reg, "L3")? },
            _ => {
                if self.dst == Store::NONE {
                    write!(f, " {}", self.src)?
                } else {
                    write!(f, ", {}", self.src)?
                }
            },
        }
        if self.imm.is_some() && !(is_l_cache(&self.dst) || is_l_cache(&self.src)) {
            if self.unsigned_imm {
                write!(f, ", {}", self.imm.unwrap() as u32)?
            } else {
                write!(f, ", {}", self.imm.unwrap())?
            }
        }
        if self.mode != Mode::None {
            write!(f, ", {}", self.mode)?;
        }
        Ok(())
    }
}

fn write_l_access(f: &mut fmt::Formatter<'_>, instr: &Instr, reg: &Store, lstore: &str) -> fmt::Result {
    if reg == &Store::Imm {
        write!(f, " {}[{}]", lstore, instr.imm.unwrap())
    } else {
        write!(f, " {}[{}{:+}]", lstore, reg, instr.imm.unwrap())
    }
}

pub struct Program {
    program: Vec<Instr>
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for instr in &self.program {
            write!(f, "{}\n", instr)?;
        }
        Ok(())
    }
}

pub fn from_bytes(bytes: Vec<m128>) -> Program {
    
    let mut program = Vec::with_capacity((bytes.len() - 8) * 2);
    
    //first 8 m128 are generated for entropy. We skip them.
    for i in 8..bytes.len() {
        let (op2, op1) = bytes[i].to_i64();
        let instr1 = decode_instruction(op1);
        let instr2 = decode_instruction(op2);
        program.push(instr1);
        program.push(instr2);
    }
    
    Program{program}
}

#[allow(overflowing_literals)]
fn decode_instruction(bytes: i64) -> Instr {
    let op = bytes & 0xFF;
    let dst = (bytes & 0xFF00) >> 8;
    let src = (bytes & 0xFF0000) >> 16;
    let modi = ((bytes & 0xFF000000) >> 24) as u8;
    let imm = ((bytes & 0xFFFFFFFF00000000) >> 32) as i32;
    
    if op < Opcode::IADD_RS as i64 {
        let dst_reg = r_reg(dst);
        let imm_val;
        if dst_reg == REG_NEEDS_DISPLACEMENT {
            imm_val = Some(imm);
        } else {
            imm_val = None;
        }
        return Instr{op: Opcode::IADD_RS, dst: dst_reg, src: r_reg(src), imm: imm_val, unsigned_imm: false, mode: mod_shft(modi), effect: nop}
    }
    if op < Opcode::IADD_M as i64 {
        return new_lcache_instr(Opcode::IADD_M, r_reg(dst), src, imm, modi);
    }
    if op < Opcode::ISUB_R as i64 {
        return new_instr(Opcode::ISUB_R, r_reg(dst), r_reg(src), imm, Mode::None);
    }
    if op < Opcode::ISUB_M as i64 {
        return new_lcache_instr(Opcode::ISUB_M, r_reg(dst), src, imm, modi);
    }
    if op < Opcode::IMUL_R as i64 {
        return new_instr(Opcode::IMUL_R, r_reg(dst), r_reg(src), imm, Mode::None);
    }
    if op < Opcode::IMUL_M as i64 {
        return new_lcache_instr(Opcode::IMUL_M, r_reg(dst), src, imm, modi);
    }
    if op < Opcode::IMULH_R as i64 {
        return Instr{op: Opcode::IMULH_R, dst: r_reg(dst), src: r_reg(src), imm: None, unsigned_imm: false, mode: Mode::None, effect: nop}
    }
    if op < Opcode::IMULH_M as i64 {
        return new_lcache_instr(Opcode::IMULH_M, r_reg(dst), src, imm, modi);
    }
    if op < Opcode::ISMULH_R as i64 {
        return new_instr(Opcode::ISMULH_R, r_reg(dst), r_reg(src), imm, Mode::None);
    }
    if op < Opcode::ISMULH_M as i64 {
        return new_lcache_instr(Opcode::ISMULH_M, r_reg(dst), src, imm, modi);
    }
    if op < Opcode::IMUL_RCP as i64 {
        let mut instr = new_imm_instr(Opcode::IMUL_RCP, r_reg(dst), imm, Mode::None);
        instr.unsigned_imm = true;
        return instr;
    }
    if op < Opcode::INEG_R as i64 {
        return new_instr(Opcode::INEG_R, r_reg(dst), Store::NONE, imm, Mode::None);
    }
    if op < Opcode::IXOR_R as i64 {
        return new_instr(Opcode::IXOR_R, r_reg(dst), r_reg(src), imm, Mode::None);
    }
    if op < Opcode::IXOR_M as i64 {
        return new_lcache_instr(Opcode::IXOR_M, r_reg(dst), src, imm, modi);
    }
    if op < Opcode::IROR_R as i64 {
        return new_instr(Opcode::IROR_R, r_reg(dst), r_reg(src), imm & 63, Mode::None);
    }
    if op < Opcode::IROL_R as i64 {
        return new_instr(Opcode::IROL_R, r_reg(dst), r_reg(src), imm & 63, Mode::None);
    }
    if op < Opcode::ISWAP_R as i64 {
        return new_instr(Opcode::ISWAP_R, r_reg(dst), r_reg(src), imm, Mode::None);
    }
    if op < Opcode::FSWAP_R as i64 {
        let dst_ix = dst % MAX_REG;
        if dst_ix >= MAX_FLOAT_REG {
            return new_instr(Opcode::FSWAP_R, e_reg_ix(dst_ix % MAX_FLOAT_REG) , Store::NONE, imm, Mode::None);
        } else {
            return new_instr(Opcode::FSWAP_R, f_reg_ix(dst_ix % MAX_FLOAT_REG), Store::NONE, imm, Mode::None);
        }
    }
    if op < Opcode::FADD_R as i64 {
        return new_instr(Opcode::FADD_R, f_reg(dst), a_reg(src), imm, Mode::None);
    }
    if op < Opcode::FADD_M as i64 {
        return new_lcache_instr(Opcode::FADD_M, f_reg(dst), src, imm, modi);
    }
    if op < Opcode::FSUB_R as i64 {
        return new_instr(Opcode::FSUB_R, f_reg(dst), a_reg(src), imm, Mode::None);
    }
    if op < Opcode::FSUB_M as i64 {
        return new_lcache_instr(Opcode::FSUB_M, f_reg(dst), src, imm, modi);
    }
    if op < Opcode::FSCAL_R as i64 {
        return new_instr(Opcode::FSCAL_R, f_reg(dst), Store::NONE, imm, Mode::None);
    }
    if op < Opcode::FMUL_R as i64 {
        return new_instr(Opcode::FMUL_R, e_reg(dst), a_reg(src), imm, Mode::None);
    }
    if op < Opcode::FDIV_M as i64 {
        return new_lcache_instr(Opcode::FDIV_M, e_reg(dst), src, imm, modi);
    }
    if op < Opcode::FSQRT_R as i64 {
        return new_instr(Opcode::FSQRT_R, e_reg(dst), Store::NONE, imm, Mode::None);
    }
    if op < Opcode::CBRANCH as i64 {
        return new_imm_instr(Opcode::CBRANCH, r_reg(dst), imm, mod_cond(modi));
    }
    if op < Opcode::CFROUND as i64 {
        return Instr{op: Opcode::CFROUND , dst: Store::NONE, src: r_reg(src), imm: Some(imm & 63), unsigned_imm: false, mode: Mode::None, effect: nop}
    }
    if op < Opcode::ISTORE as i64 {
        return Instr{op: Opcode::ISTORE, dst: l_cache(dst, modi), src: r_reg(src), imm: Some(imm), unsigned_imm: false, mode: Mode::None, effect: nop};
    }
    return new_instr(Opcode::NOP, Store::NONE, Store::NONE, imm, Mode::None);
}

fn r_reg(dst: i64) -> Store {
    match dst%MAX_REG {
        0 => Store::R0,
        1 => Store::R1,
        2 => Store::R2,
        3 => Store::R3,
        4 => Store::R4,
        5 => Store::R5,
        6 => Store::R6,
        7 => Store::R7,
        _ => Store::R0,
    }
}

fn a_reg(dst: i64) -> Store {
    match dst%MAX_FLOAT_REG {
        0 => Store::A0,
        1 => Store::A1,
        2 => Store::A2,
        3 => Store::A3,
        _ => Store::A0,
    }
}

fn e_reg(dst: i64) -> Store {
    e_reg_ix(dst%MAX_FLOAT_REG)
}

fn e_reg_ix(ix: i64) -> Store {
    match ix {
        0 => Store::E0,
        1 => Store::E1,
        2 => Store::E2,
        3 => Store::E3,
        _ => Store::E0,
    }
}

fn f_reg(dst: i64) -> Store {
    f_reg_ix(dst%MAX_FLOAT_REG)
}

fn f_reg_ix(ix: i64) -> Store {
    match ix {
        0 => Store::F0,
        1 => Store::F1,
        2 => Store::F2,
        3 => Store::F3,
        _ => Store::F0,
    }
}

fn l_cache(dst: i64, modi: u8) -> Store {
    let reg = r_reg(dst);
    let cond = mod_cond_u8(modi);
    if cond < STORE_L3_CONDITION {
        if mod_mem_u8(modi) == 0 {
            return Store::L2(Box::new(reg));
        }
        return Store::L1(Box::new(reg));
    } 
    return Store::L3(Box::new(reg));
}

fn l12_cache(src: i64, modi: u8) -> Store {
    let reg = r_reg(src);
    if mod_mem_u8(modi) == 0 {
        return Store::L2(Box::new(reg));
    }
    return Store::L1(Box::new(reg));
}

fn is_l_cache(store: &Store) -> bool {
    match store {
        Store::L1(_) => true,
        Store::L2(_) => true,
        Store::L3(_) => true,
        _ => false,
    }
}

fn mod_mem_u8(modi: u8) -> u8 {
    modi % 4 //bit 0-1
}

fn mod_cond_u8(modi: u8) -> u8 {
    modi >> 4 //bits 4-7
}

fn mod_cond(modi: u8) -> Mode {
    Mode::Cond(mod_cond_u8(modi)) 
}

fn mod_shft(modi: u8) -> Mode {
    Mode::Shft((modi >> 2) % 4)
}

pub struct State {}
pub fn nop(_state: &mut State) {}