use byteorder::{ByteOrder, LittleEndian};
use ckb_vm::instructions::{extract_opcode, i, m, rvc, Instruction, Itype, Stype, Utype};
use ckb_vm_definitions::instructions as insts;
use clap::{App, Arg};
use goblin::elf::{section_header::SHF_EXECINSTR, Elf};
use std::fs::{read, write};

fn main() {
    let matches = App::new("CKB binary patcher")
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .required(true)
                .help("Input binary filename")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("remove-a")
                .short("a")
                .long("remove-a")
                .help("Remove A instructions")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .required(true)
                .help("Output binary filename")
                .takes_value(true),
        )
        .get_matches();
    let remove_a = matches.is_present("remove-a");
    let mut data = read(matches.value_of("input").unwrap()).expect("cannot open input file!");

    let elf = Elf::parse(&data).expect("cannot parse input binary!");

    for section_header in elf.section_headers {
        if section_header.sh_flags & u64::from(SHF_EXECINSTR) != 0 {
            let mut pc = section_header.sh_offset;
            let end = section_header.sh_offset + section_header.sh_size;
            while pc < end {
                match decode_instruction(&data, pc) {
                    (Some(i), len) => {
                        pc += process_instruction(&mut data, pc, i, len);
                    }
                    (None, len) => {
                        if remove_a {
                            remove_a_instruction(&mut data, pc);
                        }
                        pc += len;
                    }
                }
            }
        }
    }

    write(matches.value_of("output").unwrap(), &data).expect("cannot write to output file!");
}

fn process_instruction(data: &mut Vec<u8>, pc: u64, instruction: Instruction, len: u64) -> u64 {
    let next_pc = pc + len;
    let op = extract_opcode(instruction);
    match op {
        insts::OP_AUIPC => {
            let i = Utype(instruction);
            if let (Some(next_instruction), next_len) = decode_instruction(&data, next_pc) {
                let next_op = extract_opcode(next_instruction);
                match next_op {
                    insts::OP_JALR => {
                        let next_i = Itype(next_instruction);
                        if next_i.rs1() == next_i.rd() && next_i.rs1() == i.rd() {
                            let destination = pc
                                .wrapping_add(i64::from(i.immediate_s()) as u64)
                                .wrapping_add(i64::from(next_i.immediate_s()) as u64);
                            let offset = destination.wrapping_sub(next_pc);
                            let masked = offset & 0xFFFFFFFFFFF00001;
                            if masked != 0 && masked != 0xFFFFFFFFFFF00000 {
                                panic!("Invalid offset: {:016x}", offset);
                            }
                            let jal_instruction = 0b1101111
                                | ((i.rd() as u32) << 7)
                                | ((((offset >> 12) & 0b_1111_1111) as u32) << 12)
                                | ((((offset >> 11) & 1) as u32) << 20)
                                | ((((offset >> 1) & 0b_1111_1111_11) as u32) << 21)
                                | ((((offset >> 20) & 1) as u32) << 31);
                            let nop_instruction = 0x00000013;
                            LittleEndian::write_u32(&mut data[pc as usize..], nop_instruction);
                            LittleEndian::write_u32(&mut data[next_pc as usize..], jal_instruction);
                            return len + next_len;
                        }
                    }
                    insts::OP_RVC_JALR => {
                        let next_i = Stype(next_instruction);
                        if next_i.rs1() == 1 && next_i.rs1() == i.rd() {
                            let destination = pc.wrapping_add(i64::from(i.immediate_s()) as u64);
                            let offset = destination.wrapping_sub(next_pc);
                            let masked = offset & 0xFFFFFFFFFFF00001;
                            if masked != 0 && masked != 0xFFFFFFFFFFF00000 {
                                panic!("Invalid offset: {:016x}", offset);
                            }
                            let jal_instruction = 0b1101111
                                | ((i.rd() as u32) << 7)
                                | ((((offset >> 12) & 0b_1111_1111) as u32) << 12)
                                | ((((offset >> 11) & 1) as u32) << 20)
                                | ((((offset >> 1) & 0b_1111_1111_11) as u32) << 21)
                                | ((((offset >> 20) & 1) as u32) << 31);
                            LittleEndian::write_u32(&mut data[pc as usize..], jal_instruction);
                            // Jump to 4 bytes earlier
                            LittleEndian::write_u16(
                                &mut data[next_pc as usize..],
                                0b_1011_1111_1111_0101,
                            );
                            return len + next_len;
                        }
                    }
                    _ => (),
                }
            }
        }
        insts::OP_JALR => {
            let i = Itype(instruction);
            if i.rs1() == i.rd() && i.rd() != 0 {
                panic!("The instruction {:016x} at {:x} will trigger a bug, see https://github.com/nervosnetwork/ckb-vm/issues/92", instruction, pc);
            }
        }
        insts::OP_RVC_JALR => {
            let i = Stype(instruction);
            if i.rs1() == 1 {
                panic!("The instruction {:016x} at {:x} will trigger a bug, see https://github.com/nervosnetwork/ckb-vm/issues/92", instruction, pc);
            }
        }
        _ => (),
    };
    len
}

fn remove_a_instruction(data: &mut Vec<u8>, pc: u64) {
    if pc >= data.len() as u64 {
        return;
    }
    let i = u32::from(LittleEndian::read_u16(&data[pc as usize..]));
    let mut len = 2;
    if i & 0x3 == 0x3 {
        len = 4;
    }
    let pc2 = pc as usize;
    if len == 4
        && (
            // amoadd.w.aq	a5,a4,(a3)
            &data[pc2..pc2 + 4] == &[0xaf, 0xa7, 0xe6, 0x04] ||
            // amoswap.d.aq	a4,a4,(a5)
            &data[pc2..pc2+4] == &[0x2f, 0xb7,  0xe7, 0x0c]
        )
    {
        data[pc2] = 0x03; // fff00503 lb a0,-1(zero)
        data[pc2 + 1] = 0x05;
        data[pc2 + 2] = 0xf0;
        data[pc2 + 3] = 0xff;
        println!(
            "instruction at {}(content 04e6a7af) is replace by \"lb a0,-1(zero)\"",
            pc2
        );
    }
}

fn decode_instruction(data: &Vec<u8>, pc: u64) -> (Option<Instruction>, u64) {
    if pc >= data.len() as u64 {
        return (None, 0);
    }
    let mut i = u32::from(LittleEndian::read_u16(&data[pc as usize..]));
    let mut len = 2;
    if i & 0x3 == 0x3 {
        i = LittleEndian::read_u32(&data[pc as usize..]);
        len = 4;
    }
    let factories = [rvc::factory::<u64>, i::factory::<u64>, m::factory::<u64>];
    for factory in &factories {
        if let Some(instruction) = factory(i) {
            return (Some(instruction), len);
        }
    }
    (None, len)
}
