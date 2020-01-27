use std::path::Path;
use llvm_sys::analysis::{LLVMVerifierFailureAction::*, LLVMVerifyModule};
use llvm_sys::core::*;
use llvm_sys::debuginfo::LLVMStripModuleDebugInfo;
use llvm_sys::initialization::*;
use llvm_sys::ir_reader::LLVMParseIRInContext;
use llvm_sys::prelude::*;
use llvm_sys::target::*;
use llvm_sys::{LLVMAttributeFunctionIndex, LLVMAttributeReturnIndex, LLVMInlineAsmDialect::*};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub fn process_ir(input: &Path, output: &Path) -> Result<(), String> {
    unsafe {
        let context = LLVMGetGlobalContext();

        LLVM_InitializeAllTargets();
        LLVM_InitializeAllTargetMCs();
        LLVM_InitializeAllAsmPrinters();
        LLVM_InitializeAllAsmParsers();

        let registry = LLVMGetGlobalPassRegistry();
        LLVMInitializeCore(registry);
        LLVMInitializeCodeGen(registry);
        LLVMInitializeScalarOpts(registry);
        LLVMInitializeVectorization(registry);

        let mut message: *mut c_char = ptr::null_mut();
        let filename = CString::new(input.to_str().unwrap()).unwrap();
        let mut buf: LLVMMemoryBufferRef = ptr::null_mut();
        LLVMCreateMemoryBufferWithContentsOfFile(
            filename.as_ptr(),
            &mut buf as *mut _,
            &mut message as *mut *mut c_char,
        );
        if !message.is_null() {
            let message = CStr::from_ptr(message);
            return Err(message.to_string_lossy().into_owned());
        }

        let mut module: LLVMModuleRef = ptr::null_mut();
        let mut message: *mut c_char = ptr::null_mut();
        LLVMParseIRInContext(
            context,
            buf,
            &mut module as *mut _,
            &mut message as *mut *mut c_char,
        );
        if !message.is_null() {
            let message = CStr::from_ptr(message);
            return Err(message.to_string_lossy().into_owned());
        }

        let no_inline = CString::new("noinline").unwrap();
        let no_inline_kind = LLVMGetEnumAttributeKindForName(no_inline.as_ptr(), "noinline".len());
        let always_inline = CString::new("alwaysinline").unwrap();
        let always_inline_kind =
            LLVMGetEnumAttributeKindForName(always_inline.as_ptr(), "alwaysinline".len());
        let always_inline_attr = LLVMCreateEnumAttribute(context, always_inline_kind, 0);
        let mut func = LLVMGetFirstFunction(module);
        let exit_str = CString::new("exit").unwrap();
        let builder = LLVMCreateBuilderInContext(context);
        let exit_sig = LLVMFunctionType(LLVMVoidTypeInContext(context), ptr::null_mut(), 0, 0);
        let exit = LLVMGetInlineAsm(
            exit_sig,
            exit_str.as_ptr() as *mut _,
            "exit".len(),
            ptr::null_mut(),
            0,
            0,
            0,
            LLVMInlineAsmDialectATT,
        );
        while func != ptr::null_mut() {
            let mut size: libc::size_t = 0;
            let name = CStr::from_ptr(LLVMGetValueName2(func, &mut size as *mut _))
                .to_str()
                .unwrap();
            if !name.starts_with("llvm.") {
                LLVMRemoveEnumAttributeAtIndex(func, LLVMAttributeFunctionIndex, no_inline_kind);
                LLVMAddAttributeAtIndex(func, LLVMAttributeFunctionIndex, always_inline_attr);
                if name == "rust_begin_unwind" {
                    let no_return = CString::new("noreturn").unwrap();
                    let no_return_kind =
                        LLVMGetEnumAttributeKindForName(no_return.as_ptr(), "noreturn".len());
                    LLVMRemoveEnumAttributeAtIndex(func, LLVMAttributeFunctionIndex, no_return_kind);
                    let block = LLVMGetLastBasicBlock(func);
                    let last = LLVMGetLastInstruction(block);
                    LLVMPositionBuilderBefore(builder, last);
                    LLVMBuildCall(
                        builder,
                        exit,
                        ptr::null_mut(),
                        0,
                        CString::new("").unwrap().as_ptr(),
                    );
                }
            }
            func = LLVMGetNextFunction(func);
        }
        LLVMStripModuleDebugInfo(module);

        let mut message: *mut c_char = ptr::null_mut();
        let ret = LLVMVerifyModule(module, LLVMPrintMessageAction, &mut message as *mut *mut _);
        if ret == 1 && !message.is_null() {
            let message = CStr::from_ptr(message);
            return Err(format!("verification failed: {}", message.to_string_lossy().into_owned()));
        }

        let mut message: *mut c_char = ptr::null_mut();
        let out = CString::new(output.to_str().unwrap()).unwrap();
        LLVMPrintModuleToFile(module, out.as_ptr(), &mut message as *mut *mut _);
        if !message.is_null() {
            let message = CStr::from_ptr(message);
            return Err(message.to_string_lossy().into_owned());
        }
    }

    Ok(())
}