//! The cpu module contains `Cpu` and implementarion for it.

#![allow(dead_code)]

use crate::bus::*;
use crate::memory::*;
use crate::trap::*;

// Machine-level CSRs.
/// Hardware thread ID.
pub const MHARTID: usize = 0xf14;
/// Machine status register.
pub const MSTATUS: usize = 0x300;
/// Machine exception delefation register.
pub const MEDELEG: usize = 0x302;
/// Machine interrupt delefation register.
pub const MIDELEG: usize = 0x303;
/// Machine interrupt-enable register.
pub const MIE: usize = 0x304;
/// Machine trap-handler base address.
pub const MTVEC: usize = 0x305;
/// Machine counter enable.
pub const MCOUNTEREN: usize = 0x306;
/// Scratch register for machine trap handlers.
pub const MSCRATCH: usize = 0x340;
/// Machine exception program counter.
pub const MEPC: usize = 0x341;
/// Machine trap cause.
pub const MCAUSE: usize = 0x342;
/// Machine bad address or instruction.
pub const MTVAL: usize = 0x343;
/// Machine interrupt pending.
pub const MIP: usize = 0x344;

// Supervisor-level CSRs.
/// Supervisor status register.
pub const SSTATUS: usize = 0x100;
/// Supervisor interrupt-enable register.
pub const SIE: usize = 0x104;
/// Supervisor trap handler base address.
pub const STVEC: usize = 0x105;
/// Scratch register for supervisor trap handlers.
pub const SSCRATCH: usize = 0x140;
/// Supervisor exception program counter.
pub const SEPC: usize = 0x141;
/// Supervisor trap cause.
pub const SCAUSE: usize = 0x142;
/// Supervisor bad address or instruction.
pub const STVAL: usize = 0x143;
/// Supervisor interrupt pending.
pub const SIP: usize = 0x144;
/// Supervisor address translation and protection.
pub const SATP: usize = 0x180;

/// The privileged mode.
#[derive(Debug, PartialEq, PartialOrd, Eq, Copy, Clone)]
pub enum Mode {
    User = 0b00,
    Supervisor = 0b01,
    Machine = 0b11,
}

/// The `Cpu` struct that contains registers, a program coutner, system bus that connects
/// peripheral devices, and control and status registers.
pub struct Cpu {
    /// 32 64-bit integer registers.
    pub regs: [u64; 32],
    /// Program counter to hold the the memory address of the next instruction that would be executed.
    pub pc: u64,
    /// The current privilege mode.
    pub mode: Mode,
    /// System bus that transfers data between CPU and peripheral devices.
    pub bus: Bus,
    /// Control and status registers. RISC-V ISA sets aside a 12-bit encoding space (csr[11:0]) for
    /// up to 4096 CSRs.
    pub csrs: [u64; 4096],
}

impl Cpu {
    /// Create a new `Cpu` object.
    pub fn new(binary: Vec<u8>) -> Self {
        // The stack pointer (SP) must be set up at first.
        let mut regs = [0; 32];
        regs[2] = MEMORY_BASE + MEMORY_SIZE;

        Self {
            regs,
            // The program counter starts from the start address of a memory.
            pc: MEMORY_BASE,
            mode: Mode::Machine,
            bus: Bus::new(binary),
            csrs: [0; 4096],
        }
    }

    /// Print values in all registers (x0-x31).
    pub fn dump_registers(&self) {
        let mut output = String::from("");
        let abi = [
            "zero", " ra ", " sp ", " gp ", " tp ", " t0 ", " t1 ", " t2 ", " s0 ", " s1 ", " a0 ",
            " a1 ", " a2 ", " a3 ", " a4 ", " a5 ", " a6 ", " a7 ", " s2 ", " s3 ", " s4 ", " s5 ",
            " s6 ", " s7 ", " s8 ", " s9 ", " s10", " s11", " t3 ", " t4 ", " t5 ", " t6 ",
        ];
        for i in (0..32).step_by(4) {
            output = format!(
                "{}\n{}",
                output,
                format!(
                    "x{:02}({})={:>#18x} x{:02}({})={:>#18x} x{:02}({})={:>#18x} x{:02}({})={:>#18x}",
                    i,
                    abi[i],
                    self.regs[i],
                    i + 1,
                    abi[i + 1],
                    self.regs[i + 1],
                    i + 2,
                    abi[i + 2],
                    self.regs[i + 2],
                    i + 3,
                    abi[i + 3],
                    self.regs[i + 3],
                )
            );
        }
        println!("{}", output);
    }

    /// Print values in some csrs.
    pub fn dump_csrs(&self) {
        let output = format!(
            "{}\n{}",
            format!(
                "mstatus={:>#18x} mtvec={:>#18x} mepc={:>#18x} mcause={:>#18x}",
                self.load_csr(MSTATUS),
                self.load_csr(MTVEC),
                self.load_csr(MEPC),
                self.load_csr(MCAUSE),
            ),
            format!(
                "sstatus={:>#18x} stvec={:>#18x} sepc={:>#18x} scause={:>#18x}",
                self.load_csr(SSTATUS),
                self.load_csr(STVEC),
                self.load_csr(SEPC),
                self.load_csr(SCAUSE),
            ),
        );
        println!("{}", output);
    }

    /// Load a value from a CSR.
    pub fn load_csr(&self, addr: usize) -> u64 {
        match addr {
            SIE => self.csrs[MIE] & self.csrs[MIDELEG],
            _ => self.csrs[addr],
        }
    }

    /// Store a value to a CSR.
    pub fn store_csr(&mut self, addr: usize, value: u64) {
        match addr {
            SIE => {
                self.csrs[MIE] =
                    (self.csrs[MIE] & !self.csrs[MIDELEG]) | (value & self.csrs[MIDELEG]);
            }
            _ => self.csrs[addr] = value,
        }
    }

    /// Load a value from a memory.
    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        self.bus.load(addr, size)
    }

    /// Store a value to a memory.
    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        self.bus.store(addr, size, value)
    }

    /// Get an instruction from the memory.
    pub fn fetch(&mut self) -> Result<u64, Exception> {
        match self.bus.load(self.pc, 32) {
            Ok(inst) => Ok(inst),
            Err(_e) => Err(Exception::InstructionAccessFault),
        }
    }

    /// Execute an instruction after decoding. Return true if an error happens, otherwise false.
    pub fn execute(&mut self, inst: u64) -> Result<(), Exception> {
        let opcode = inst & 0x0000007f;
        let rd = ((inst & 0x00000f80) >> 7) as usize;
        let rs1 = ((inst & 0x000f8000) >> 15) as usize;
        let rs2 = ((inst & 0x01f00000) >> 20) as usize;
        let funct3 = (inst & 0x00007000) >> 12;
        let funct7 = (inst & 0xfe000000) >> 25;

        // Emulate that register x0 is hardwired with all bits equal to 0.
        self.regs[0] = 0;

        match opcode {
            0x03 => {
                // imm[11:0] = inst[31:20]
                let imm = ((inst as i32 as i64) >> 20) as u64;
                let addr = self.regs[rs1].wrapping_add(imm);
                match funct3 {
                    0x0 => {
                        // lb
                        let val = self.load(addr, 8)?;
                        self.regs[rd] = val as i8 as i64 as u64;
                    }
                    0x1 => {
                        // lh
                        let val = self.load(addr, 16)?;
                        self.regs[rd] = val as i16 as i64 as u64;
                    }
                    0x2 => {
                        // lw
                        let val = self.load(addr, 32)?;
                        self.regs[rd] = val as i32 as i64 as u64;
                    }
                    0x3 => {
                        // ld
                        let val = self.load(addr, 64)?;
                        self.regs[rd] = val;
                    }
                    0x4 => {
                        // lbu
                        let val = self.load(addr, 8)?;
                        self.regs[rd] = val;
                    }
                    0x5 => {
                        // lhu
                        let val = self.load(addr, 16)?;
                        self.regs[rd] = val;
                    }
                    0x6 => {
                        // lwu
                        let val = self.load(addr, 32)?;
                        self.regs[rd] = val;
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x0f => {
                // A fence instruction does nothing because this emulator executes an
                // instruction sequentially on a single thread.
                match funct3 {
                    0x0 => {} // fence
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x13 => {
                // imm[11:0] = inst[31:20]
                let imm = ((inst & 0xfff00000) as i32 as i64 >> 20) as u64;
                // "The shift amount is encoded in the lower 6 bits of the I-immediate field for RV64I."
                let shamt = (imm & 0x3f) as u32;
                match funct3 {
                    0x0 => {
                        // addi
                        self.regs[rd] = self.regs[rs1].wrapping_add(imm);
                    }
                    0x1 => {
                        // slli
                        self.regs[rd] = self.regs[rs1] << shamt;
                    }
                    0x2 => {
                        // slti
                        self.regs[rd] = if (self.regs[rs1] as i64) < (imm as i64) {
                            1
                        } else {
                            0
                        };
                    }
                    0x3 => {
                        // sltiu
                        self.regs[rd] = if self.regs[rs1] < imm { 1 } else { 0 };
                    }
                    0x4 => {
                        // xori
                        self.regs[rd] = self.regs[rs1] ^ imm;
                    }
                    0x5 => {
                        match funct7 >> 1 {
                            // srli
                            0x00 => self.regs[rd] = self.regs[rs1].wrapping_shr(shamt),
                            // srai
                            0x10 => {
                                self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64
                            }
                            _ => {}
                        }
                    }
                    0x6 => self.regs[rd] = self.regs[rs1] | imm, // ori
                    0x7 => self.regs[rd] = self.regs[rs1] & imm, // andi
                    _ => {}
                }
            }
            0x17 => {
                // auipc
                let imm = (inst & 0xfffff000) as i32 as i64 as u64;
                self.regs[rd] = self.pc.wrapping_add(imm).wrapping_sub(4);
            }
            0x1b => {
                let imm = ((inst as i32 as i64) >> 20) as u64;
                // "SLLIW, SRLIW, and SRAIW encodings with imm[5] ̸= 0 are reserved."
                let shamt = (imm & 0x1f) as u32;
                match funct3 {
                    0x0 => {
                        // addiw
                        self.regs[rd] = self.regs[rs1].wrapping_add(imm) as i32 as i64 as u64;
                    }
                    0x1 => {
                        // slliw
                        self.regs[rd] = self.regs[rs1].wrapping_shl(shamt) as i32 as i64 as u64;
                    }
                    0x5 => {
                        match funct7 {
                            0x00 => {
                                // srliw
                                self.regs[rd] = (self.regs[rs1] as u32).wrapping_shr(shamt) as i32
                                    as i64 as u64;
                            }
                            0x20 => {
                                // sraiw
                                self.regs[rd] =
                                    (self.regs[rs1] as i32).wrapping_shr(shamt) as i64 as u64;
                            }
                            _ => {
                                println!(
                                    "not implemented yet: opcode {:#x} funct7 {:#x}",
                                    opcode, funct7
                                );
                                return Err(Exception::IllegalInstruction);
                            }
                        }
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x23 => {
                // imm[11:5|4:0] = inst[31:25|11:7]
                let imm = (((inst & 0xfe000000) as i32 as i64 >> 20) as u64) | ((inst >> 7) & 0x1f);
                let addr = self.regs[rs1].wrapping_add(imm);
                match funct3 {
                    0x0 => self.store(addr, 8, self.regs[rs2])?,  // sb
                    0x1 => self.store(addr, 16, self.regs[rs2])?, // sh
                    0x2 => self.store(addr, 32, self.regs[rs2])?, // sw
                    0x3 => self.store(addr, 64, self.regs[rs2])?, // sd
                    _ => {}
                }
            }
            0x2f => {
                // RV64A: "A" standard extension for atomic instructions
                let funct5 = (funct7 & 0b1111100) >> 2;
                let _aq = (funct7 & 0b0000010) >> 1; // acquire access
                let _rl = funct7 & 0b0000001; // release access
                match (funct3, funct5) {
                    (0x2, 0x00) => {
                        // amoadd.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, t.wrapping_add(self.regs[rs2]))?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x00) => {
                        // amoadd.d
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, t.wrapping_add(self.regs[rs2]))?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x01) => {
                        // amoswap.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, self.regs[rs2])?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x01) => {
                        // amoswap.d
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2])?;
                        self.regs[rd] = t;
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x} funct7 {:#x}",
                            opcode, funct3, funct7
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x33 => {
                // "SLL, SRL, and SRA perform logical left, logical right, and arithmetic right
                // shifts on the value in register rs1 by the shift amount held in register rs2.
                // In RV64I, only the low 6 bits of rs2 are considered for the shift amount."
                let shamt = ((self.regs[rs2] & 0x3f) as u64) as u32;
                match (funct3, funct7) {
                    (0x0, 0x00) => {
                        // add
                        self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
                    }
                    (0x0, 0x01) => {
                        // mul
                        self.regs[rd] = self.regs[rs1].wrapping_mul(self.regs[rs2]);
                    }
                    (0x0, 0x20) => {
                        // sub
                        self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]);
                    }
                    (0x1, 0x00) => {
                        // sll
                        self.regs[rd] = self.regs[rs1].wrapping_shl(shamt);
                    }
                    (0x2, 0x00) => {
                        // slt
                        self.regs[rd] = if (self.regs[rs1] as i64) < (self.regs[rs2] as i64) {
                            1
                        } else {
                            0
                        };
                    }
                    (0x3, 0x00) => {
                        // sltu
                        self.regs[rd] = if self.regs[rs1] < self.regs[rs2] {
                            1
                        } else {
                            0
                        };
                    }
                    (0x4, 0x00) => {
                        // xor
                        self.regs[rd] = self.regs[rs1] ^ self.regs[rs2];
                    }
                    (0x5, 0x00) => {
                        // srl
                        self.regs[rd] = self.regs[rs1].wrapping_shr(shamt);
                    }
                    (0x5, 0x20) => {
                        // sra
                        self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64;
                    }
                    (0x6, 0x00) => {
                        // or
                        self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                    }
                    (0x7, 0x00) => {
                        // and
                        self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x} funct7 {:#x}",
                            opcode, funct3, funct7
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x37 => {
                // lui
                self.regs[rd] = (inst & 0xfffff000) as i32 as i64 as u64;
            }
            0x3b => {
                // "The shift amount is given by rs2[4:0]."
                let shamt = (self.regs[rs2] & 0x1f) as u32;
                match (funct3, funct7) {
                    (0x0, 0x00) => {
                        // addw
                        self.regs[rd] =
                            self.regs[rs1].wrapping_add(self.regs[rs2]) as i32 as i64 as u64;
                    }
                    (0x0, 0x20) => {
                        // subw
                        self.regs[rd] =
                            ((self.regs[rs1].wrapping_sub(self.regs[rs2])) as i32) as u64;
                    }
                    (0x1, 0x00) => {
                        // sllw
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shl(shamt) as i32 as u64;
                    }
                    (0x5, 0x00) => {
                        // srlw
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shr(shamt) as i32 as u64;
                    }
                    (0x5, 0x01) => {
                        // divu
                        self.regs[rd] = match self.regs[rs2] {
                            0 => {
                                // TODO: Set DZ (Divide by Zero) in the FCSR csr flag to 1.
                                0xffffffff_ffffffff
                            }
                            _ => {
                                let dividend = self.regs[rs1];
                                let divisor = self.regs[rs2];
                                dividend.wrapping_div(divisor)
                            }
                        };
                    }
                    (0x5, 0x20) => {
                        // sraw
                        self.regs[rd] = ((self.regs[rs1] as i32) >> (shamt as i32)) as u64;
                    }
                    (0x7, 0x01) => {
                        // remuw
                        self.regs[rd] = match self.regs[rs2] {
                            0 => self.regs[rs1],
                            _ => {
                                let dividend = self.regs[rs1] as u32;
                                let divisor = self.regs[rs2] as u32;
                                dividend.wrapping_rem(divisor) as i32 as u64
                            }
                        };
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x} funct7 {:#x}",
                            opcode, funct3, funct7
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x63 => {
                // imm[12|10:5|4:1|11] = inst[31|30:25|11:8|7]
                let imm = (((inst & 0x80000000) as i32 as i64 >> 19) as u64)
                    | ((inst & 0x80) << 4) // imm[11]
                    | ((inst >> 20) & 0x7e0) // imm[10:5]
                    | ((inst >> 7) & 0x1e); // imm[4:1]

                match funct3 {
                    0x0 => {
                        // beq
                        if self.regs[rs1] == self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x1 => {
                        // bne
                        if self.regs[rs1] != self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x4 => {
                        // blt
                        if (self.regs[rs1] as i64) < (self.regs[rs2] as i64) {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x5 => {
                        // bge
                        if (self.regs[rs1] as i64) >= (self.regs[rs2] as i64) {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x6 => {
                        // bltu
                        if self.regs[rs1] < self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x7 => {
                        // bgeu
                        if self.regs[rs1] >= self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x67 => {
                // jalr
                // Note: Don't add 4 because the pc already moved on.
                let t = self.pc;

                let imm = ((((inst & 0xfff00000) as i32) as i64) >> 20) as u64;
                self.pc = (self.regs[rs1].wrapping_add(imm)) & !1;

                self.regs[rd] = t;
            }
            0x6f => {
                // jal
                self.regs[rd] = self.pc;

                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = (((inst & 0x80000000) as i32 as i64 >> 11) as u64) // imm[20]
                    | (inst & 0xff000) // imm[19:12]
                    | ((inst >> 9) & 0x800) // imm[11]
                    | ((inst >> 20) & 0x7fe); // imm[10:1]

                self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
            }
            0x73 => {
                let csr_addr = ((inst & 0xfff00000) >> 20) as usize;
                match funct3 {
                    0x0 => {
                        match (rs2, funct7) {
                            (0x0, 0x0) => {
                                // ecall
                                // Makes a request of the execution environment by raising an
                                // environment call exception.
                                match self.mode {
                                    Mode::User => {
                                        return Err(Exception::EnvironmentCallFromUMode);
                                    }
                                    Mode::Supervisor => {
                                        return Err(Exception::EnvironmentCallFromSMode);
                                    }
                                    Mode::Machine => {
                                        return Err(Exception::EnvironmentCallFromMMode);
                                    }
                                }
                            }
                            (0x1, 0x0) => {
                                // ebreak
                                // Makes a request of the debugger bu raising a Breakpoint
                                // exception.
                                return Err(Exception::Breakpoint);
                            }
                            (0x2, 0x8) => {
                                // sret
                                // The SRET instruction returns from a supervisor-mode exception
                                // handler. It does the following operations:
                                // - Sets the pc to CSRs[sepc].
                                // - Sets the privilege mode to CSRs[sstatus].SPP.
                                // - Sets CSRs[sstatus].SIE to CSRs[sstatus].SPIE.
                                // - Sets CSRs[sstatus].SPIE to 1.
                                // - Sets CSRs[sstatus].SPP to 0.
                                self.pc = self.load_csr(SEPC);
                                // When the SRET instruction is executed to return from the trap
                                // handler, the privilege level is set to user mode if the SPP
                                // bit is 0, or supervisor mode if the SPP bit is 1. The SPP bit
                                // is the 8th of the SSTATUS csr.
                                self.mode = match (self.load_csr(SSTATUS) >> 8) & 1 {
                                    1 => Mode::Supervisor,
                                    _ => Mode::User,
                                };
                                // The SPIE bit is the 5th and the SIE bit is the 1st of the
                                // SSTATUS csr.
                                self.store_csr(
                                    SSTATUS,
                                    if ((self.load_csr(SSTATUS) >> 5) & 1) == 1 {
                                        self.load_csr(SSTATUS) | (1 << 1)
                                    } else {
                                        self.load_csr(SSTATUS) & !(1 << 1)
                                    },
                                );
                                self.store_csr(SSTATUS, self.load_csr(SSTATUS) | (1 << 5));
                                self.store_csr(SSTATUS, self.load_csr(SSTATUS) & !(1 << 8));
                            }
                            (0x2, 0x18) => {
                                // mret
                                // The MRET instruction returns from a machine-mode exception
                                // handler. It does the following operations:
                                // - Sets the pc to CSRs[mepc].
                                // - Sets the privilege mode to CSRs[mstatus].MPP.
                                // - Sets CSRs[mstatus].MIE to CSRs[mstatus].MPIE.
                                // - Sets CSRs[mstatus].MPIE to 1.
                                // - Sets CSRs[mstatus].MPP to 0.
                                self.pc = self.load_csr(MEPC);
                                // MPP is two bits wide at [11..12] of the MSTATUS csr.
                                self.mode = match (self.load_csr(MSTATUS) >> 11) & 0b11 {
                                    2 => Mode::Machine,
                                    1 => Mode::Supervisor,
                                    _ => Mode::User,
                                };
                                // The MPIE bit is the 7th and the MIE bit is the 3rd of the
                                // MSTATUS csr.
                                self.store_csr(
                                    MSTATUS,
                                    if ((self.load_csr(MSTATUS) >> 7) & 1) == 1 {
                                        self.load_csr(MSTATUS) | (1 << 3)
                                    } else {
                                        self.load_csr(MSTATUS) & !(1 << 3)
                                    },
                                );
                                self.store_csr(MSTATUS, self.load_csr(MSTATUS) | (1 << 7));
                                self.store_csr(MSTATUS, self.load_csr(MSTATUS) & !(0b11 << 11));
                            }
                            (_, 0x9) => {
                                // sfence.vma
                                // Do nothing.
                            }
                            _ => {
                                println!(
                                    "not implemented yet: opcode {:#x} funct3 {:#x} funct7 {:#x}",
                                    opcode, funct3, funct7
                                );
                                return Err(Exception::IllegalInstruction);
                            }
                        }
                    }
                    0x1 => {
                        // csrrw
                        let t = self.load_csr(csr_addr);
                        self.store_csr(csr_addr, self.regs[rs1]);
                        self.regs[rd] = t;
                    }
                    0x2 => {
                        // csrrs
                        let t = self.load_csr(csr_addr);
                        self.store_csr(csr_addr, t | self.regs[rs1]);
                        self.regs[rd] = t;
                    }
                    0x3 => {
                        // csrrc
                        let t = self.load_csr(csr_addr);
                        self.store_csr(csr_addr, t & (!self.regs[rs1]));
                        self.regs[rd] = t;
                    }
                    0x5 => {
                        // csrrwi
                        let zimm = rs1 as u64;
                        self.regs[rd] = self.load_csr(csr_addr);
                        self.store_csr(csr_addr, zimm);
                    }
                    0x6 => {
                        // csrrsi
                        let zimm = rs1 as u64;
                        let t = self.load_csr(csr_addr);
                        self.store_csr(csr_addr, t | zimm);
                        self.regs[rd] = t;
                    }
                    0x7 => {
                        // csrrci
                        let zimm = rs1 as u64;
                        let t = self.load_csr(csr_addr);
                        self.store_csr(csr_addr, t & (!zimm));
                        self.regs[rd] = t;
                    }
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            _ => {
                dbg!(format!("not implemented yet: opcode {:#x}", opcode));
                return Err(Exception::IllegalInstruction);
            }
        }
        return Ok(());
    }
}
