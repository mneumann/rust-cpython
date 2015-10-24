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

extern crate num;

use libc::{c_long, c_double};
use python::{Python, PythonObject, ToPythonPointer};
use err::{self, PyResult, PyErr};
use super::object::PyObject;
use super::exc;
use ffi;
use conversion::{ToPyObject, ExtractPyObject};

/// Represents a Python `int` object.
///
/// Note that in Python 2.x, `int` and `long` are different types.
/// When rust-cpython is compiled for Python 3.x,
/// `PyInt` and `PyLong` are aliases for the same type, which
/// corresponds to a Python `int`.
///
/// You can usually avoid directly working with this type
/// by using [ToPyObject](trait.ToPyObject.html)
/// and [extract](struct.PyObject.html#method.extract)
/// with the primitive Rust integer types.
#[cfg(feature="python27-sys")]
pub struct PyInt<'p>(PyObject<'p>);
#[cfg(feature="python27-sys")]
pyobject_newtype!(PyInt, PyInt_Check, PyInt_Type);

/// In Python 2.x, represents a Python `long` object.
/// In Python 3.x, represents a Python `int` object.
/// Both `PyInt` and `PyLong` refer to the same type on Python 3.x.
///
/// You can usually avoid directly working with this type
/// by using [ToPyObject](trait.ToPyObject.html)
/// and [extract](struct.PyObject.html#method.extract)
/// with the primitive Rust integer types.
pub struct PyLong<'p>(PyObject<'p>);
pyobject_newtype!(PyLong, PyLong_Check, PyLong_Type);

/// Represents a Python `float` object.
///
/// You can usually avoid directly working with this type
/// by using [ToPyObject](trait.ToPyObject.html)
/// and [extract](struct.PyObject.html#method.extract)
/// with `f32`/`f64`.
pub struct PyFloat<'p>(PyObject<'p>);
pyobject_newtype!(PyFloat, PyFloat_Check, PyFloat_Type);

#[cfg(feature="python27-sys")]
impl <'p> PyInt<'p> {
    /// Creates a new Python `int` object.
    pub fn new(py: Python<'p>, val: c_long) -> PyInt<'p> {
        unsafe {
            err::cast_from_owned_ptr_or_panic(py, ffi::PyInt_FromLong(val))
        }
    }

    /// Gets the value of this integer.
    pub fn value(&self) -> c_long {
        unsafe { ffi::PyInt_AS_LONG(self.as_ptr()) }
    }
}


impl <'p> PyFloat<'p> {
    /// Creates a new Python `float` object.
    pub fn new(py: Python<'p>, val: c_double) -> PyFloat<'p> {
        unsafe {
            err::cast_from_owned_ptr_or_panic(py, ffi::PyFloat_FromDouble(val))
        }
    }

    /// Gets the value of this float.
    pub fn value(&self) -> c_double {
        unsafe { ffi::PyFloat_AsDouble(self.as_ptr()) }
    }
}

macro_rules! int_fits_c_long(
    ($rust_type:ty) => (
        #[cfg(feature="python27-sys")]
        impl <'p> ToPyObject<'p> for $rust_type {
            type ObjectType = PyInt<'p>;

            fn to_py_object(&self, py: Python<'p>) -> PyInt<'p> {
                unsafe {
                    err::cast_from_owned_ptr_or_panic(py,
                        ffi::PyInt_FromLong(*self as c_long))
                }
            }
        }

        #[cfg(feature="python3-sys")]
        impl <'p> ToPyObject<'p> for $rust_type {
            type ObjectType = PyLong<'p>;

            fn to_py_object(&self, py: Python<'p>) -> PyLong<'p> {
                unsafe {
                    err::cast_from_owned_ptr_or_panic(py,
                        ffi::PyLong_FromLong(*self as c_long))
                }
            }
        }

        extract!(obj to $rust_type => {
            let py = obj.python();
            let val = unsafe { ffi::PyLong_AsLong(obj.as_ptr()) };
            if val == -1 && PyErr::occurred(py) {
                return Err(PyErr::fetch(py));
            }
            match num::traits::cast::<c_long, $rust_type>(val) {
                Some(v) => Ok(v),
                None => Err(overflow_error(py))
            }
        });
    )
);


macro_rules! int_fits_larger_int(
    ($rust_type:ty, $larger_type:ty) => (
        impl <'p> ToPyObject<'p> for $rust_type {
            type ObjectType = <$larger_type as ToPyObject<'p>>::ObjectType;

            #[inline]
            fn to_py_object(&self, py: Python<'p>) -> <$larger_type as ToPyObject<'p>>::ObjectType {
                (*self as $larger_type).to_py_object(py)
            }
        }

        extract!(obj to $rust_type => {
            let py = obj.python();
            let val = try!(obj.extract::<$larger_type>());
            match num::traits::cast::<$larger_type, $rust_type>(val) {
                Some(v) => Ok(v),
                None => Err(overflow_error(py))
            }
        });
    )
);


fn err_if_invalid_value<'p, T: PartialEq, F: Fn() -> T>
   (obj: &PyObject<'p>, invalid_value: T, func: F) -> PyResult<'p, T> {
    let py = obj.python();
    let v = func();
    if v == invalid_value && PyErr::occurred(py) {
        Err(PyErr::fetch(py))
    } else {
        Ok(v)
    }
}

macro_rules! int_convert_u64_or_i64 (
    ($rust_type:ty, $pylong_from_ll_or_ull:expr, $pylong_as_ull_or_ull:expr) => (
        impl <'p> ToPyObject<'p> for $rust_type {
            #[cfg(feature="python27-sys")]
            type ObjectType = PyObject<'p>;

            #[cfg(feature="python3-sys")]
            type ObjectType = PyLong<'p>;

            #[cfg(feature="python27-sys")]
            fn to_py_object(&self, py: Python<'p>) -> PyObject<'p> {
                unsafe {
                    let ptr = match num::traits::cast::<$rust_type, c_long>(*self) {
                        Some(v) => ffi::PyInt_FromLong(v),
                        None => $pylong_from_ll_or_ull(*self)
                    };
                    err::from_owned_ptr_or_panic(py, ptr)
                }
            }

            #[cfg(feature="python3-sys")]
            fn to_py_object(&self, py: Python<'p>) -> PyLong<'p> {
                unsafe {
                    err::cast_from_owned_ptr_or_panic(py, $pylong_from_ll_or_ull(*self))
                }
            }
        }

        impl <'python, 'prepared>
            ExtractPyObject<'python, 'prepared> for $rust_type
        {
            type Prepared = $rust_type;

            #[cfg(feature="python27-sys")]
            fn prepare_extract(obj: &PyObject<'python>) -> PyResult<'python, $rust_type> {
                let py = obj.python();
                let ptr = obj.as_ptr();

                unsafe {
                    if ffi::PyLong_Check(ptr) != 0 {
                        err_if_invalid_value(&obj, !0, || $pylong_as_ull_or_ull(obj.as_ptr()) )
                    } else if ffi::PyInt_Check(ptr) != 0 {
                        match num::traits::cast::<c_long, $rust_type>(ffi::PyInt_AS_LONG(ptr)) {
                            Some(v) => Ok(v),
                            None => Err(overflow_error(py))
                        }
                    } else {
                        let num = try!(err::result_from_owned_ptr(py, ffi::PyNumber_Long(ptr)));
                        err_if_invalid_value(&num, !0, || $pylong_as_ull_or_ull(num.as_ptr()) )
                    }
                }
            }

            #[cfg(feature="python3-sys")]
            fn prepare_extract(obj: &PyObject<'python>) -> PyResult<'python, $rust_type> {
                let py = obj.python();
                let ptr = obj.as_ptr();
                unsafe {
                    if ffi::PyLong_Check(ptr) != 0 {
                        err_if_invalid_value(&obj, !0, || $pylong_as_ull_or_ull(obj.as_ptr()) )
                    } else {
                        let num = try!(err::result_from_owned_ptr(py, ffi::PyNumber_Long(ptr)));
                        err_if_invalid_value(&num, !0, || $pylong_as_ull_or_ull(num.as_ptr()) )
                    }
                }
            }

            #[inline]
            fn extract(prepared: &'prepared $rust_type) -> PyResult<'python, $rust_type> {
                Ok(*prepared)
            }
        }
    )
);


int_fits_c_long!(i8);
int_fits_c_long!(u8);
int_fits_c_long!(i16);
int_fits_c_long!(u16);
int_fits_c_long!(i32);

// If c_long is 64-bits, we can use more types with int_fits_c_long!:
#[cfg(all(target_pointer_width="64", not(target_os="windows")))]
int_fits_c_long!(u32);
#[cfg(any(target_pointer_width="32", target_os="windows"))]
int_fits_larger_int!(u32, u64);

#[cfg(all(target_pointer_width="64", not(target_os="windows")))]
int_fits_c_long!(i64);

// manual implementation for i64 on systems with 32-bit long
#[cfg(any(target_pointer_width="32", target_os="windows"))]
int_convert_u64_or_i64!(i64, ffi::PyLong_FromLongLong, ffi::PyLong_AsLongLong);

#[cfg(all(target_pointer_width="64", not(target_os="windows")))]
int_fits_c_long!(isize);
#[cfg(any(target_pointer_width="32", target_os="windows"))]
int_fits_larger_int!(isize, i64);

int_fits_larger_int!(usize, u64);

// u64 has a manual implementation as it never fits into signed long
int_convert_u64_or_i64!(u64, ffi::PyLong_FromUnsignedLongLong, ffi::PyLong_AsUnsignedLongLong);

impl <'p> ToPyObject<'p> for f64 {
    type ObjectType = PyFloat<'p>;

    fn to_py_object(&self, py: Python<'p>) -> PyFloat<'p> {
       PyFloat::new(py, *self)
    }
}

extract!(obj to f64 => {
    let py = obj.python();
    let v = unsafe { ffi::PyFloat_AsDouble(obj.as_ptr()) };
    if v == -1.0 && PyErr::occurred(py) {
        Err(PyErr::fetch(py))
    } else {
        Ok(v)
    }
});

fn overflow_error(py: Python) -> PyErr {
    PyErr::new_lazy_init(py.get_type::<exc::OverflowError>(), None)
}

impl <'p> ToPyObject<'p> for f32 {
    type ObjectType = PyFloat<'p>;

    fn to_py_object(&self, py: Python<'p>) -> PyFloat<'p> {
       PyFloat::new(py, *self as f64)
    }
}

extract!(obj to f32 => {
    Ok(try!(obj.extract::<f64>()) as f32)
});

#[cfg(test)]
mod test {
    use std;
    use python::{Python, PythonObject};
    use conversion::ToPyObject;

    macro_rules! num_to_py_object_and_back (
        ($func_name:ident, $t1:ty, $t2:ty) => (
            #[test]
            fn $func_name() {
                let gil = Python::acquire_gil();
                let py = gil.python();
                let val = 123 as $t1;
                let obj = val.to_py_object(py).into_object();
                assert_eq!(obj.extract::<$t2>().unwrap(), val as $t2);
            }
        )
    );

    num_to_py_object_and_back!(to_from_f64, f64, f64);
    num_to_py_object_and_back!(to_from_f32, f32, f32);
    num_to_py_object_and_back!(to_from_i8,   i8,  i8);
    num_to_py_object_and_back!(to_from_u8,   u8,  u8);
    num_to_py_object_and_back!(to_from_i16, i16, i16);
    num_to_py_object_and_back!(to_from_u16, u16, u16);
    num_to_py_object_and_back!(to_from_i32, i32, i32);
    num_to_py_object_and_back!(to_from_u32, u32, u32);
    num_to_py_object_and_back!(to_from_i64, i64, i64);
    num_to_py_object_and_back!(to_from_u64, u64, u64);
    num_to_py_object_and_back!(to_from_isize, isize, isize);
    num_to_py_object_and_back!(to_from_usize, usize, usize);
    num_to_py_object_and_back!(float_to_i32, f64, i32);
    num_to_py_object_and_back!(float_to_u32, f64, u32);
    num_to_py_object_and_back!(float_to_i64, f64, i64);
    num_to_py_object_and_back!(float_to_u64, f64, u64);
    num_to_py_object_and_back!(int_to_float, i32, f64);

    #[test]
    fn test_u32_max() {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let v = std::u32::MAX;
        let obj = v.to_py_object(py).into_object();
        assert_eq!(v, obj.extract::<u32>().unwrap());
        assert_eq!(v as u64, obj.extract::<u64>().unwrap());
        assert!(obj.extract::<i32>().is_err());
    }
    
    #[test]
    fn test_i64_max() {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let v = std::i64::MAX;
        let obj = v.to_py_object(py).into_object();
        assert_eq!(v, obj.extract::<i64>().unwrap());
        assert_eq!(v as u64, obj.extract::<u64>().unwrap());
        assert!(obj.extract::<u32>().is_err());
    }
    
    #[test]
    fn test_i64_min() {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let v = std::i64::MIN;
        let obj = v.to_py_object(py).into_object();
        assert_eq!(v, obj.extract::<i64>().unwrap());
        assert!(obj.extract::<i32>().is_err());
        assert!(obj.extract::<u64>().is_err());
    }
    
    #[test]
    fn test_u64_max() {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let v = std::u64::MAX;
        let obj = v.to_py_object(py).into_object();
        println!("{:?}", obj);
        assert_eq!(v, obj.extract::<u64>().unwrap());
        assert!(obj.extract::<i64>().is_err());
    }
}
