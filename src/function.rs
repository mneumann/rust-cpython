// Copyright (c) 2015 Daniel Grunwald
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this
// software and associated documentation files (the "Software"), to deal in the Software
// without restriction, including without limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
// to whom the Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or
// substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
// PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
// FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::ptr;
use python::{Python, PythonObject};
use objects::{PyObject, PyTuple, PyDict, PyString, exc};
use conversion::ToPyObject;
use ffi;
use err::{self, PyResult};

/// Creates a Python callable object that invokes a Rust function.
///
/// As arguments, takes the name of a rust function with the signature
/// `for<'p> fn(Python<'p>, &PyTuple<'p>) -> PyResult<'p, T>`
/// for some `T` that implements `ToPyObject`.
///
/// Returns a type that implements `ToPyObject` by producing a Python callable.
///
/// See `py_module_initializer!` for example usage.
#[macro_export]
macro_rules! py_fn {
    ($f: ident) => ( interpolate_idents! {{
        unsafe extern "C" fn [ wrap_ $f ](
            _slf: *mut $crate::_detail::ffi::PyObject,
            args: *mut $crate::_detail::ffi::PyObject,
            kwargs: *mut $crate::_detail::ffi::PyObject)
        -> *mut $crate::_detail::ffi::PyObject
        {
            let _guard = $crate::_detail::PanicGuard::with_message("Rust panic in py_fn!");
            let py = $crate::_detail::bounded_assume_gil_acquired(&args);
            let args = $crate::PyObject::from_borrowed_ptr(py, args);
            let args = <$crate::PyTuple as $crate::PythonObject>::unchecked_downcast_from(args);
            let kwargs = match $crate::PyObject::from_borrowed_ptr_opt(py, kwargs) {
                Some(kwargs) => Some(<$crate::PyDict as $crate::PythonObject>::unchecked_downcast_from(kwargs)),
                None => None
            };
            match $f(py, &args, kwargs.as_ref()) {
                Ok(val) => {
                    let obj = $crate::ToPyObject::into_py_object(val, py);
                    return $crate::PythonObject::into_object(obj).steal_ptr();
                }
                Err(e) => {
                    e.restore(py);
                    return ::std::ptr::null_mut();
                }
            }
        }
        static mut [ method_def_ $f ]: $crate::_detail::ffi::PyMethodDef = $crate::_detail::ffi::PyMethodDef {
            //ml_name: bytes!(stringify!($f), "\0"),
            ml_name: 0 as *const $crate::_detail::libc::c_char,
            ml_meth: None,
            ml_flags: $crate::_detail::ffi::METH_VARARGS | $crate::_detail::ffi::METH_KEYWORDS,
            ml_doc: 0 as *const $crate::_detail::libc::c_char
        };
        unsafe {
            [ method_def_ $f ].ml_name = concat!(stringify!($f), "\0").as_ptr() as *const _;
            [ method_def_ $f ].ml_meth = Some(
                std::mem::transmute::<$crate::_detail::ffi::PyCFunctionWithKeywords,
                                      $crate::_detail::ffi::PyCFunction>([ wrap_ $f ])
            );
            $crate::_detail::py_fn_impl(&mut [ method_def_ $f ])
        }
    }});
    ($f: ident ( $( $pname:ident : $ptype:ty ),* ) ) => ( interpolate_idents! {{
        unsafe extern "C" fn [ wrap_ $f ](
            _slf: *mut $crate::_detail::ffi::PyObject,
            args: *mut $crate::_detail::ffi::PyObject,
            kwargs: *mut $crate::_detail::ffi::PyObject)
        -> *mut $crate::_detail::ffi::PyObject
        {
            let _guard = $crate::_detail::PanicGuard::with_message("Rust panic in py_fn!");
            let py = $crate::_detail::bounded_assume_gil_acquired(&args);
            let args = $crate::PyObject::from_borrowed_ptr(py, args);
            let args = <$crate::PyTuple as $crate::PythonObject>::unchecked_downcast_from(args);
            let kwargs = match $crate::PyObject::from_borrowed_ptr_opt(py, kwargs) {
                Some(kwargs) => Some(<$crate::PyDict as $crate::PythonObject>::unchecked_downcast_from(kwargs)),
                None => None
            };
            match py_argparse!(py, Some(stringify!($f)), &args, kwargs.as_ref(),
                    ( $($pname : $ptype),* ) { $f( py, $($pname),* ) })
            {
                Ok(val) => {
                    let obj = $crate::ToPyObject::into_py_object(val, py);
                    return $crate::PythonObject::into_object(obj).steal_ptr();
                }
                Err(e) => {
                    e.restore(py);
                    return ::std::ptr::null_mut();
                }
            }
        }
        static mut [ method_def_ $f ]: $crate::_detail::ffi::PyMethodDef = $crate::_detail::ffi::PyMethodDef {
            //ml_name: bytes!(stringify!($f), "\0"),
            ml_name: 0 as *const $crate::_detail::libc::c_char,
            ml_meth: None,
            ml_flags: $crate::_detail::ffi::METH_VARARGS | $crate::_detail::ffi::METH_KEYWORDS,
            ml_doc: 0 as *const $crate::_detail::libc::c_char
        };
        unsafe {
            [ method_def_ $f ].ml_name = concat!(stringify!($f), "\0").as_ptr() as *const _;
            [ method_def_ $f ].ml_meth = Some(
                std::mem::transmute::<$crate::_detail::ffi::PyCFunctionWithKeywords,
                                      $crate::_detail::ffi::PyCFunction>([ wrap_ $f ])
            );
            $crate::_detail::py_fn_impl(&mut [ method_def_ $f ])
        }
    }});
}

/// Result type of the `py_fn!()` macro.
///
/// Use the `ToPyObject` implementation to create a python callable object.
pub struct PyFn(*mut ffi::PyMethodDef);

#[inline]
pub unsafe fn py_fn_impl(def: *mut ffi::PyMethodDef) -> PyFn {
    PyFn(def)
}

impl ToPyObject for PyFn {
    type ObjectType = PyObject;

    fn to_py_object(&self, py: Python) -> PyObject {
        unsafe {
            err::from_owned_ptr_or_panic(py, ffi::PyCFunction_New(self.0, ptr::null_mut()))
        }
    }
}

// Tests for this file are in tests/test_function.rs

