// An image's size from its header (without decoding it), turned the way
// its EXIF says: "4032 × 3024", or "" if it can't be read.
#pragma once
#include "rust/cxx.h"

rust::String imageSize(rust::Str path);
