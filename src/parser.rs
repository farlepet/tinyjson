use std::char;
use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::str::FromStr;

use crate::JsonValue;

/// Parse error.
///
/// ```
/// use tinyjson::{JsonParser, JsonParseError};
/// let error = JsonParser::new("[1, 2, 3".chars()).parse().unwrap_err();
/// assert!(matches!(error, JsonParseError{..}));
/// ```
#[derive(Debug)]
pub struct JsonParseError {
    msg: String,
    line: usize,
    col: usize,
}

impl JsonParseError {
    fn new(msg: String, line: usize, col: usize) -> JsonParseError {
        JsonParseError { msg, line, col }
    }

    /// Get the line numbr where the parse error happened. This value is 1-based.
    ///
    /// ```
    /// use tinyjson::{JsonParser, JsonParseError};
    /// let error = JsonParser::new("[1, 2, 3".chars()).parse().unwrap_err();
    /// assert_eq!(error.line(), 1);
    /// ```
    pub fn line(&self) -> usize {
        self.line
    }

    /// Get the column numbr where the parse error happened. This value is 1-based.
    ///
    /// ```
    /// use tinyjson::{JsonParser, JsonParseError};
    /// let error = JsonParser::new("[1, 2, 3".chars()).parse().unwrap_err();
    /// assert_eq!(error.column(), 8);
    /// ```
    pub fn column(&self) -> usize {
        self.col
    }
}

impl fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at line:{}, col:{}: {}",
            self.line, self.col, &self.msg,
        )
    }
}

impl std::error::Error for JsonParseError {}

/// Convenient type alias for parse results.
pub type JsonParseResult = Result<JsonValue, JsonParseError>;

// Note: char::is_ascii_whitespace is not available because some characters are not defined as
// whitespace character in JSON spec. For example, U+000C FORM FEED is whitespace in Rust but
// it isn't in JSON.
fn is_whitespace(c: char) -> bool {
    match c {
        '\u{0020}' | '\u{000a}' | '\u{000d}' | '\u{0009}' => true,
        _ => false,
    }
}

// The Eisel-Lemire ParseFloat algorithm
//
// - Paper: https://arxiv.org/abs/2101.11408 "Number Parsing at a Gigabyte per Second"
// - Explanation: https://nigeltao.github.io/blog/2020/eisel-lemire.html
// - Reference implementations:
//   - Original C++ implementation: https://github.com/lemire/fast_double_parser/blob/644bef4306059d3be01a04e77d3cc84b379c596f/include/fast_double_parser.h#L840
//   - C re-implementation: https://github.com/google/wuffs/blob/e915543f0987b0bef4ba8eb0ddc34d617f9ce870/internal/cgen/base/floatconv-submodule-code.c#L989
//   - C++ re-implementation: https://github.com/abseil/abseil-cpp/blob/625a18016d6208c6c0419697cb6caa3f23ce31bc/absl/strings/charconv.cc#L669
//   - Go re-implementation: https://github.com/golang/go/blob/119f679a3bd2e60cfc990920f82fd1a5cb006f4c/src/strconv/eisel_lemire.go#L25
//   - Rust re-implementation: https://github.com/rust-lang/rust/blob/e94fab69d020d75517cb55fafacb2d270ad6e0ac/library/core/src/num/dec2flt/lemire.rs#L4
fn eisel_lemire(mut man: u64, exp10: i32, neg: bool) -> Option<f64> {
    const POW10_MIN_EXP10: i32 = -348;
    const POW10_MAX_EXP10: i32 = 347;
    const DETAILED_POWERS_OF_TEN: [(u64, u64); (-POW10_MIN_EXP10 + POW10_MAX_EXP10 + 1) as usize] = [
        (0x1732c869cd60e453, 0xfa8fd5a0081c0288), // 1e-348
        (0x0e7fbd42205c8eb4, 0x9c99e58405118195), // 1e-347
        (0x521fac92a873b261, 0xc3c05ee50655e1fa), // 1e-346
        (0xe6a797b752909ef9, 0xf4b0769e47eb5a78), // 1e-345
        (0x9028bed2939a635c, 0x98ee4a22ecf3188b), // 1e-344
        (0x7432ee873880fc33, 0xbf29dcaba82fdeae), // 1e-343
        (0x113faa2906a13b3f, 0xeef453d6923bd65a), // 1e-342
        (0x4ac7ca59a424c507, 0x9558b4661b6565f8), // 1e-341
        (0x5d79bcf00d2df649, 0xbaaee17fa23ebf76), // 1e-340
        (0xf4d82c2c107973dc, 0xe95a99df8ace6f53), // 1e-339
        (0x79071b9b8a4be869, 0x91d8a02bb6c10594), // 1e-338
        (0x9748e2826cdee284, 0xb64ec836a47146f9), // 1e-337
        (0xfd1b1b2308169b25, 0xe3e27a444d8d98b7), // 1e-336
        (0xfe30f0f5e50e20f7, 0x8e6d8c6ab0787f72), // 1e-335
        (0xbdbd2d335e51a935, 0xb208ef855c969f4f), // 1e-334
        (0xad2c788035e61382, 0xde8b2b66b3bc4723), // 1e-333
        (0x4c3bcb5021afcc31, 0x8b16fb203055ac76), // 1e-332
        (0xdf4abe242a1bbf3d, 0xaddcb9e83c6b1793), // 1e-331
        (0xd71d6dad34a2af0d, 0xd953e8624b85dd78), // 1e-330
        (0x8672648c40e5ad68, 0x87d4713d6f33aa6b), // 1e-329
        (0x680efdaf511f18c2, 0xa9c98d8ccb009506), // 1e-328
        (0x0212bd1b2566def2, 0xd43bf0effdc0ba48), // 1e-327
        (0x014bb630f7604b57, 0x84a57695fe98746d), // 1e-326
        (0x419ea3bd35385e2d, 0xa5ced43b7e3e9188), // 1e-325
        (0x52064cac828675b9, 0xcf42894a5dce35ea), // 1e-324
        (0x7343efebd1940993, 0x818995ce7aa0e1b2), // 1e-323
        (0x1014ebe6c5f90bf8, 0xa1ebfb4219491a1f), // 1e-322
        (0xd41a26e077774ef6, 0xca66fa129f9b60a6), // 1e-321
        (0x8920b098955522b4, 0xfd00b897478238d0), // 1e-320
        (0x55b46e5f5d5535b0, 0x9e20735e8cb16382), // 1e-319
        (0xeb2189f734aa831d, 0xc5a890362fddbc62), // 1e-318
        (0xa5e9ec7501d523e4, 0xf712b443bbd52b7b), // 1e-317
        (0x47b233c92125366e, 0x9a6bb0aa55653b2d), // 1e-316
        (0x999ec0bb696e840a, 0xc1069cd4eabe89f8), // 1e-315
        (0xc00670ea43ca250d, 0xf148440a256e2c76), // 1e-314
        (0x380406926a5e5728, 0x96cd2a865764dbca), // 1e-313
        (0xc605083704f5ecf2, 0xbc807527ed3e12bc), // 1e-312
        (0xf7864a44c633682e, 0xeba09271e88d976b), // 1e-311
        (0x7ab3ee6afbe0211d, 0x93445b8731587ea3), // 1e-310
        (0x5960ea05bad82964, 0xb8157268fdae9e4c), // 1e-309
        (0x6fb92487298e33bd, 0xe61acf033d1a45df), // 1e-308
        (0xa5d3b6d479f8e056, 0x8fd0c16206306bab), // 1e-307
        (0x8f48a4899877186c, 0xb3c4f1ba87bc8696), // 1e-306
        (0x331acdabfe94de87, 0xe0b62e2929aba83c), // 1e-305
        (0x9ff0c08b7f1d0b14, 0x8c71dcd9ba0b4925), // 1e-304
        (0x07ecf0ae5ee44dd9, 0xaf8e5410288e1b6f), // 1e-303
        (0xc9e82cd9f69d6150, 0xdb71e91432b1a24a), // 1e-302
        (0xbe311c083a225cd2, 0x892731ac9faf056e), // 1e-301
        (0x6dbd630a48aaf406, 0xab70fe17c79ac6ca), // 1e-300
        (0x092cbbccdad5b108, 0xd64d3d9db981787d), // 1e-299
        (0x25bbf56008c58ea5, 0x85f0468293f0eb4e), // 1e-298
        (0xaf2af2b80af6f24e, 0xa76c582338ed2621), // 1e-297
        (0x1af5af660db4aee1, 0xd1476e2c07286faa), // 1e-296
        (0x50d98d9fc890ed4d, 0x82cca4db847945ca), // 1e-295
        (0xe50ff107bab528a0, 0xa37fce126597973c), // 1e-294
        (0x1e53ed49a96272c8, 0xcc5fc196fefd7d0c), // 1e-293
        (0x25e8e89c13bb0f7a, 0xff77b1fcbebcdc4f), // 1e-292
        (0x77b191618c54e9ac, 0x9faacf3df73609b1), // 1e-291
        (0xd59df5b9ef6a2417, 0xc795830d75038c1d), // 1e-290
        (0x4b0573286b44ad1d, 0xf97ae3d0d2446f25), // 1e-289
        (0x4ee367f9430aec32, 0x9becce62836ac577), // 1e-288
        (0x229c41f793cda73f, 0xc2e801fb244576d5), // 1e-287
        (0x6b43527578c1110f, 0xf3a20279ed56d48a), // 1e-286
        (0x830a13896b78aaa9, 0x9845418c345644d6), // 1e-285
        (0x23cc986bc656d553, 0xbe5691ef416bd60c), // 1e-284
        (0x2cbfbe86b7ec8aa8, 0xedec366b11c6cb8f), // 1e-283
        (0x7bf7d71432f3d6a9, 0x94b3a202eb1c3f39), // 1e-282
        (0xdaf5ccd93fb0cc53, 0xb9e08a83a5e34f07), // 1e-281
        (0xd1b3400f8f9cff68, 0xe858ad248f5c22c9), // 1e-280
        (0x23100809b9c21fa1, 0x91376c36d99995be), // 1e-279
        (0xabd40a0c2832a78a, 0xb58547448ffffb2d), // 1e-278
        (0x16c90c8f323f516c, 0xe2e69915b3fff9f9), // 1e-277
        (0xae3da7d97f6792e3, 0x8dd01fad907ffc3b), // 1e-276
        (0x99cd11cfdf41779c, 0xb1442798f49ffb4a), // 1e-275
        (0x40405643d711d583, 0xdd95317f31c7fa1d), // 1e-274
        (0x482835ea666b2572, 0x8a7d3eef7f1cfc52), // 1e-273
        (0xda3243650005eecf, 0xad1c8eab5ee43b66), // 1e-272
        (0x90bed43e40076a82, 0xd863b256369d4a40), // 1e-271
        (0x5a7744a6e804a291, 0x873e4f75e2224e68), // 1e-270
        (0x711515d0a205cb36, 0xa90de3535aaae202), // 1e-269
        (0x0d5a5b44ca873e03, 0xd3515c2831559a83), // 1e-268
        (0xe858790afe9486c2, 0x8412d9991ed58091), // 1e-267
        (0x626e974dbe39a872, 0xa5178fff668ae0b6), // 1e-266
        (0xfb0a3d212dc8128f, 0xce5d73ff402d98e3), // 1e-265
        (0x7ce66634bc9d0b99, 0x80fa687f881c7f8e), // 1e-264
        (0x1c1fffc1ebc44e80, 0xa139029f6a239f72), // 1e-263
        (0xa327ffb266b56220, 0xc987434744ac874e), // 1e-262
        (0x4bf1ff9f0062baa8, 0xfbe9141915d7a922), // 1e-261
        (0x6f773fc3603db4a9, 0x9d71ac8fada6c9b5), // 1e-260
        (0xcb550fb4384d21d3, 0xc4ce17b399107c22), // 1e-259
        (0x7e2a53a146606a48, 0xf6019da07f549b2b), // 1e-258
        (0x2eda7444cbfc426d, 0x99c102844f94e0fb), // 1e-257
        (0xfa911155fefb5308, 0xc0314325637a1939), // 1e-256
        (0x793555ab7eba27ca, 0xf03d93eebc589f88), // 1e-255
        (0x4bc1558b2f3458de, 0x96267c7535b763b5), // 1e-254
        (0x9eb1aaedfb016f16, 0xbbb01b9283253ca2), // 1e-253
        (0x465e15a979c1cadc, 0xea9c227723ee8bcb), // 1e-252
        (0x0bfacd89ec191ec9, 0x92a1958a7675175f), // 1e-251
        (0xcef980ec671f667b, 0xb749faed14125d36), // 1e-250
        (0x82b7e12780e7401a, 0xe51c79a85916f484), // 1e-249
        (0xd1b2ecb8b0908810, 0x8f31cc0937ae58d2), // 1e-248
        (0x861fa7e6dcb4aa15, 0xb2fe3f0b8599ef07), // 1e-247
        (0x67a791e093e1d49a, 0xdfbdcece67006ac9), // 1e-246
        (0xe0c8bb2c5c6d24e0, 0x8bd6a141006042bd), // 1e-245
        (0x58fae9f773886e18, 0xaecc49914078536d), // 1e-244
        (0xaf39a475506a899e, 0xda7f5bf590966848), // 1e-243
        (0x6d8406c952429603, 0x888f99797a5e012d), // 1e-242
        (0xc8e5087ba6d33b83, 0xaab37fd7d8f58178), // 1e-241
        (0xfb1e4a9a90880a64, 0xd5605fcdcf32e1d6), // 1e-240
        (0x5cf2eea09a55067f, 0x855c3be0a17fcd26), // 1e-239
        (0xf42faa48c0ea481e, 0xa6b34ad8c9dfc06f), // 1e-238
        (0xf13b94daf124da26, 0xd0601d8efc57b08b), // 1e-237
        (0x76c53d08d6b70858, 0x823c12795db6ce57), // 1e-236
        (0x54768c4b0c64ca6e, 0xa2cb1717b52481ed), // 1e-235
        (0xa9942f5dcf7dfd09, 0xcb7ddcdda26da268), // 1e-234
        (0xd3f93b35435d7c4c, 0xfe5d54150b090b02), // 1e-233
        (0xc47bc5014a1a6daf, 0x9efa548d26e5a6e1), // 1e-232
        (0x359ab6419ca1091b, 0xc6b8e9b0709f109a), // 1e-231
        (0xc30163d203c94b62, 0xf867241c8cc6d4c0), // 1e-230
        (0x79e0de63425dcf1d, 0x9b407691d7fc44f8), // 1e-229
        (0x985915fc12f542e4, 0xc21094364dfb5636), // 1e-228
        (0x3e6f5b7b17b2939d, 0xf294b943e17a2bc4), // 1e-227
        (0xa705992ceecf9c42, 0x979cf3ca6cec5b5a), // 1e-226
        (0x50c6ff782a838353, 0xbd8430bd08277231), // 1e-225
        (0xa4f8bf5635246428, 0xece53cec4a314ebd), // 1e-224
        (0x871b7795e136be99, 0x940f4613ae5ed136), // 1e-223
        (0x28e2557b59846e3f, 0xb913179899f68584), // 1e-222
        (0x331aeada2fe589cf, 0xe757dd7ec07426e5), // 1e-221
        (0x3ff0d2c85def7621, 0x9096ea6f3848984f), // 1e-220
        (0x0fed077a756b53a9, 0xb4bca50b065abe63), // 1e-219
        (0xd3e8495912c62894, 0xe1ebce4dc7f16dfb), // 1e-218
        (0x64712dd7abbbd95c, 0x8d3360f09cf6e4bd), // 1e-217
        (0xbd8d794d96aacfb3, 0xb080392cc4349dec), // 1e-216
        (0xecf0d7a0fc5583a0, 0xdca04777f541c567), // 1e-215
        (0xf41686c49db57244, 0x89e42caaf9491b60), // 1e-214
        (0x311c2875c522ced5, 0xac5d37d5b79b6239), // 1e-213
        (0x7d633293366b828b, 0xd77485cb25823ac7), // 1e-212
        (0xae5dff9c02033197, 0x86a8d39ef77164bc), // 1e-211
        (0xd9f57f830283fdfc, 0xa8530886b54dbdeb), // 1e-210
        (0xd072df63c324fd7b, 0xd267caa862a12d66), // 1e-209
        (0x4247cb9e59f71e6d, 0x8380dea93da4bc60), // 1e-208
        (0x52d9be85f074e608, 0xa46116538d0deb78), // 1e-207
        (0x67902e276c921f8b, 0xcd795be870516656), // 1e-206
        (0x00ba1cd8a3db53b6, 0x806bd9714632dff6), // 1e-205
        (0x80e8a40eccd228a4, 0xa086cfcd97bf97f3), // 1e-204
        (0x6122cd128006b2cd, 0xc8a883c0fdaf7df0), // 1e-203
        (0x796b805720085f81, 0xfad2a4b13d1b5d6c), // 1e-202
        (0xcbe3303674053bb0, 0x9cc3a6eec6311a63), // 1e-201
        (0xbedbfc4411068a9c, 0xc3f490aa77bd60fc), // 1e-200
        (0xee92fb5515482d44, 0xf4f1b4d515acb93b), // 1e-199
        (0x751bdd152d4d1c4a, 0x991711052d8bf3c5), // 1e-198
        (0xd262d45a78a0635d, 0xbf5cd54678eef0b6), // 1e-197
        (0x86fb897116c87c34, 0xef340a98172aace4), // 1e-196
        (0xd45d35e6ae3d4da0, 0x9580869f0e7aac0e), // 1e-195
        (0x8974836059cca109, 0xbae0a846d2195712), // 1e-194
        (0x2bd1a438703fc94b, 0xe998d258869facd7), // 1e-193
        (0x7b6306a34627ddcf, 0x91ff83775423cc06), // 1e-192
        (0x1a3bc84c17b1d542, 0xb67f6455292cbf08), // 1e-191
        (0x20caba5f1d9e4a93, 0xe41f3d6a7377eeca), // 1e-190
        (0x547eb47b7282ee9c, 0x8e938662882af53e), // 1e-189
        (0xe99e619a4f23aa43, 0xb23867fb2a35b28d), // 1e-188
        (0x6405fa00e2ec94d4, 0xdec681f9f4c31f31), // 1e-187
        (0xde83bc408dd3dd04, 0x8b3c113c38f9f37e), // 1e-186
        (0x9624ab50b148d445, 0xae0b158b4738705e), // 1e-185
        (0x3badd624dd9b0957, 0xd98ddaee19068c76), // 1e-184
        (0xe54ca5d70a80e5d6, 0x87f8a8d4cfa417c9), // 1e-183
        (0x5e9fcf4ccd211f4c, 0xa9f6d30a038d1dbc), // 1e-182
        (0x7647c3200069671f, 0xd47487cc8470652b), // 1e-181
        (0x29ecd9f40041e073, 0x84c8d4dfd2c63f3b), // 1e-180
        (0xf468107100525890, 0xa5fb0a17c777cf09), // 1e-179
        (0x7182148d4066eeb4, 0xcf79cc9db955c2cc), // 1e-178
        (0xc6f14cd848405530, 0x81ac1fe293d599bf), // 1e-177
        (0xb8ada00e5a506a7c, 0xa21727db38cb002f), // 1e-176
        (0xa6d90811f0e4851c, 0xca9cf1d206fdc03b), // 1e-175
        (0x908f4a166d1da663, 0xfd442e4688bd304a), // 1e-174
        (0x9a598e4e043287fe, 0x9e4a9cec15763e2e), // 1e-173
        (0x40eff1e1853f29fd, 0xc5dd44271ad3cdba), // 1e-172
        (0xd12bee59e68ef47c, 0xf7549530e188c128), // 1e-171
        (0x82bb74f8301958ce, 0x9a94dd3e8cf578b9), // 1e-170
        (0xe36a52363c1faf01, 0xc13a148e3032d6e7), // 1e-169
        (0xdc44e6c3cb279ac1, 0xf18899b1bc3f8ca1), // 1e-168
        (0x29ab103a5ef8c0b9, 0x96f5600f15a7b7e5), // 1e-167
        (0x7415d448f6b6f0e7, 0xbcb2b812db11a5de), // 1e-166
        (0x111b495b3464ad21, 0xebdf661791d60f56), // 1e-165
        (0xcab10dd900beec34, 0x936b9fcebb25c995), // 1e-164
        (0x3d5d514f40eea742, 0xb84687c269ef3bfb), // 1e-163
        (0x0cb4a5a3112a5112, 0xe65829b3046b0afa), // 1e-162
        (0x47f0e785eaba72ab, 0x8ff71a0fe2c2e6dc), // 1e-161
        (0x59ed216765690f56, 0xb3f4e093db73a093), // 1e-160
        (0x306869c13ec3532c, 0xe0f218b8d25088b8), // 1e-159
        (0x1e414218c73a13fb, 0x8c974f7383725573), // 1e-158
        (0xe5d1929ef90898fa, 0xafbd2350644eeacf), // 1e-157
        (0xdf45f746b74abf39, 0xdbac6c247d62a583), // 1e-156
        (0x6b8bba8c328eb783, 0x894bc396ce5da772), // 1e-155
        (0x066ea92f3f326564, 0xab9eb47c81f5114f), // 1e-154
        (0xc80a537b0efefebd, 0xd686619ba27255a2), // 1e-153
        (0xbd06742ce95f5f36, 0x8613fd0145877585), // 1e-152
        (0x2c48113823b73704, 0xa798fc4196e952e7), // 1e-151
        (0xf75a15862ca504c5, 0xd17f3b51fca3a7a0), // 1e-150
        (0x9a984d73dbe722fb, 0x82ef85133de648c4), // 1e-149
        (0xc13e60d0d2e0ebba, 0xa3ab66580d5fdaf5), // 1e-148
        (0x318df905079926a8, 0xcc963fee10b7d1b3), // 1e-147
        (0xfdf17746497f7052, 0xffbbcfe994e5c61f), // 1e-146
        (0xfeb6ea8bedefa633, 0x9fd561f1fd0f9bd3), // 1e-145
        (0xfe64a52ee96b8fc0, 0xc7caba6e7c5382c8), // 1e-144
        (0x3dfdce7aa3c673b0, 0xf9bd690a1b68637b), // 1e-143
        (0x06bea10ca65c084e, 0x9c1661a651213e2d), // 1e-142
        (0x486e494fcff30a62, 0xc31bfa0fe5698db8), // 1e-141
        (0x5a89dba3c3efccfa, 0xf3e2f893dec3f126), // 1e-140
        (0xf89629465a75e01c, 0x986ddb5c6b3a76b7), // 1e-139
        (0xf6bbb397f1135823, 0xbe89523386091465), // 1e-138
        (0x746aa07ded582e2c, 0xee2ba6c0678b597f), // 1e-137
        (0xa8c2a44eb4571cdc, 0x94db483840b717ef), // 1e-136
        (0x92f34d62616ce413, 0xba121a4650e4ddeb), // 1e-135
        (0x77b020baf9c81d17, 0xe896a0d7e51e1566), // 1e-134
        (0x0ace1474dc1d122e, 0x915e2486ef32cd60), // 1e-133
        (0x0d819992132456ba, 0xb5b5ada8aaff80b8), // 1e-132
        (0x10e1fff697ed6c69, 0xe3231912d5bf60e6), // 1e-131
        (0xca8d3ffa1ef463c1, 0x8df5efabc5979c8f), // 1e-130
        (0xbd308ff8a6b17cb2, 0xb1736b96b6fd83b3), // 1e-129
        (0xac7cb3f6d05ddbde, 0xddd0467c64bce4a0), // 1e-128
        (0x6bcdf07a423aa96b, 0x8aa22c0dbef60ee4), // 1e-127
        (0x86c16c98d2c953c6, 0xad4ab7112eb3929d), // 1e-126
        (0xe871c7bf077ba8b7, 0xd89d64d57a607744), // 1e-125
        (0x11471cd764ad4972, 0x87625f056c7c4a8b), // 1e-124
        (0xd598e40d3dd89bcf, 0xa93af6c6c79b5d2d), // 1e-123
        (0x4aff1d108d4ec2c3, 0xd389b47879823479), // 1e-122
        (0xcedf722a585139ba, 0x843610cb4bf160cb), // 1e-121
        (0xc2974eb4ee658828, 0xa54394fe1eedb8fe), // 1e-120
        (0x733d226229feea32, 0xce947a3da6a9273e), // 1e-119
        (0x0806357d5a3f525f, 0x811ccc668829b887), // 1e-118
        (0xca07c2dcb0cf26f7, 0xa163ff802a3426a8), // 1e-117
        (0xfc89b393dd02f0b5, 0xc9bcff6034c13052), // 1e-116
        (0xbbac2078d443ace2, 0xfc2c3f3841f17c67), // 1e-115
        (0xd54b944b84aa4c0d, 0x9d9ba7832936edc0), // 1e-114
        (0x0a9e795e65d4df11, 0xc5029163f384a931), // 1e-113
        (0x4d4617b5ff4a16d5, 0xf64335bcf065d37d), // 1e-112
        (0x504bced1bf8e4e45, 0x99ea0196163fa42e), // 1e-111
        (0xe45ec2862f71e1d6, 0xc06481fb9bcf8d39), // 1e-110
        (0x5d767327bb4e5a4c, 0xf07da27a82c37088), // 1e-109
        (0x3a6a07f8d510f86f, 0x964e858c91ba2655), // 1e-108
        (0x890489f70a55368b, 0xbbe226efb628afea), // 1e-107
        (0x2b45ac74ccea842e, 0xeadab0aba3b2dbe5), // 1e-106
        (0x3b0b8bc90012929d, 0x92c8ae6b464fc96f), // 1e-105
        (0x09ce6ebb40173744, 0xb77ada0617e3bbcb), // 1e-104
        (0xcc420a6a101d0515, 0xe55990879ddcaabd), // 1e-103
        (0x9fa946824a12232d, 0x8f57fa54c2a9eab6), // 1e-102
        (0x47939822dc96abf9, 0xb32df8e9f3546564), // 1e-101
        (0x59787e2b93bc56f7, 0xdff9772470297ebd), // 1e-100
        (0x57eb4edb3c55b65a, 0x8bfbea76c619ef36), // 1e-99
        (0xede622920b6b23f1, 0xaefae51477a06b03), // 1e-98
        (0xe95fab368e45eced, 0xdab99e59958885c4), // 1e-97
        (0x11dbcb0218ebb414, 0x88b402f7fd75539b), // 1e-96
        (0xd652bdc29f26a119, 0xaae103b5fcd2a881), // 1e-95
        (0x4be76d3346f0495f, 0xd59944a37c0752a2), // 1e-94
        (0x6f70a4400c562ddb, 0x857fcae62d8493a5), // 1e-93
        (0xcb4ccd500f6bb952, 0xa6dfbd9fb8e5b88e), // 1e-92
        (0x7e2000a41346a7a7, 0xd097ad07a71f26b2), // 1e-91
        (0x8ed400668c0c28c8, 0x825ecc24c873782f), // 1e-90
        (0x728900802f0f32fa, 0xa2f67f2dfa90563b), // 1e-89
        (0x4f2b40a03ad2ffb9, 0xcbb41ef979346bca), // 1e-88
        (0xe2f610c84987bfa8, 0xfea126b7d78186bc), // 1e-87
        (0x0dd9ca7d2df4d7c9, 0x9f24b832e6b0f436), // 1e-86
        (0x91503d1c79720dbb, 0xc6ede63fa05d3143), // 1e-85
        (0x75a44c6397ce912a, 0xf8a95fcf88747d94), // 1e-84
        (0xc986afbe3ee11aba, 0x9b69dbe1b548ce7c), // 1e-83
        (0xfbe85badce996168, 0xc24452da229b021b), // 1e-82
        (0xfae27299423fb9c3, 0xf2d56790ab41c2a2), // 1e-81
        (0xdccd879fc967d41a, 0x97c560ba6b0919a5), // 1e-80
        (0x5400e987bbc1c920, 0xbdb6b8e905cb600f), // 1e-79
        (0x290123e9aab23b68, 0xed246723473e3813), // 1e-78
        (0xf9a0b6720aaf6521, 0x9436c0760c86e30b), // 1e-77
        (0xf808e40e8d5b3e69, 0xb94470938fa89bce), // 1e-76
        (0xb60b1d1230b20e04, 0xe7958cb87392c2c2), // 1e-75
        (0xb1c6f22b5e6f48c2, 0x90bd77f3483bb9b9), // 1e-74
        (0x1e38aeb6360b1af3, 0xb4ecd5f01a4aa828), // 1e-73
        (0x25c6da63c38de1b0, 0xe2280b6c20dd5232), // 1e-72
        (0x579c487e5a38ad0e, 0x8d590723948a535f), // 1e-71
        (0x2d835a9df0c6d851, 0xb0af48ec79ace837), // 1e-70
        (0xf8e431456cf88e65, 0xdcdb1b2798182244), // 1e-69
        (0x1b8e9ecb641b58ff, 0x8a08f0f8bf0f156b), // 1e-68
        (0xe272467e3d222f3f, 0xac8b2d36eed2dac5), // 1e-67
        (0x5b0ed81dcc6abb0f, 0xd7adf884aa879177), // 1e-66
        (0x98e947129fc2b4e9, 0x86ccbb52ea94baea), // 1e-65
        (0x3f2398d747b36224, 0xa87fea27a539e9a5), // 1e-64
        (0x8eec7f0d19a03aad, 0xd29fe4b18e88640e), // 1e-63
        (0x1953cf68300424ac, 0x83a3eeeef9153e89), // 1e-62
        (0x5fa8c3423c052dd7, 0xa48ceaaab75a8e2b), // 1e-61
        (0x3792f412cb06794d, 0xcdb02555653131b6), // 1e-60
        (0xe2bbd88bbee40bd0, 0x808e17555f3ebf11), // 1e-59
        (0x5b6aceaeae9d0ec4, 0xa0b19d2ab70e6ed6), // 1e-58
        (0xf245825a5a445275, 0xc8de047564d20a8b), // 1e-57
        (0xeed6e2f0f0d56712, 0xfb158592be068d2e), // 1e-56
        (0x55464dd69685606b, 0x9ced737bb6c4183d), // 1e-55
        (0xaa97e14c3c26b886, 0xc428d05aa4751e4c), // 1e-54
        (0xd53dd99f4b3066a8, 0xf53304714d9265df), // 1e-53
        (0xe546a8038efe4029, 0x993fe2c6d07b7fab), // 1e-52
        (0xde98520472bdd033, 0xbf8fdb78849a5f96), // 1e-51
        (0x963e66858f6d4440, 0xef73d256a5c0f77c), // 1e-50
        (0xdde7001379a44aa8, 0x95a8637627989aad), // 1e-49
        (0x5560c018580d5d52, 0xbb127c53b17ec159), // 1e-48
        (0xaab8f01e6e10b4a6, 0xe9d71b689dde71af), // 1e-47
        (0xcab3961304ca70e8, 0x9226712162ab070d), // 1e-46
        (0x3d607b97c5fd0d22, 0xb6b00d69bb55c8d1), // 1e-45
        (0x8cb89a7db77c506a, 0xe45c10c42a2b3b05), // 1e-44
        (0x77f3608e92adb242, 0x8eb98a7a9a5b04e3), // 1e-43
        (0x55f038b237591ed3, 0xb267ed1940f1c61c), // 1e-42
        (0x6b6c46dec52f6688, 0xdf01e85f912e37a3), // 1e-41
        (0x2323ac4b3b3da015, 0x8b61313bbabce2c6), // 1e-40
        (0xabec975e0a0d081a, 0xae397d8aa96c1b77), // 1e-39
        (0x96e7bd358c904a21, 0xd9c7dced53c72255), // 1e-38
        (0x7e50d64177da2e54, 0x881cea14545c7575), // 1e-37
        (0xdde50bd1d5d0b9e9, 0xaa242499697392d2), // 1e-36
        (0x955e4ec64b44e864, 0xd4ad2dbfc3d07787), // 1e-35
        (0xbd5af13bef0b113e, 0x84ec3c97da624ab4), // 1e-34
        (0xecb1ad8aeacdd58e, 0xa6274bbdd0fadd61), // 1e-33
        (0x67de18eda5814af2, 0xcfb11ead453994ba), // 1e-32
        (0x80eacf948770ced7, 0x81ceb32c4b43fcf4), // 1e-31
        (0xa1258379a94d028d, 0xa2425ff75e14fc31), // 1e-30
        (0x096ee45813a04330, 0xcad2f7f5359a3b3e), // 1e-29
        (0x8bca9d6e188853fc, 0xfd87b5f28300ca0d), // 1e-28
        (0x775ea264cf55347d, 0x9e74d1b791e07e48), // 1e-27
        (0x95364afe032a819d, 0xc612062576589dda), // 1e-26
        (0x3a83ddbd83f52204, 0xf79687aed3eec551), // 1e-25
        (0xc4926a9672793542, 0x9abe14cd44753b52), // 1e-24
        (0x75b7053c0f178293, 0xc16d9a0095928a27), // 1e-23
        (0x5324c68b12dd6338, 0xf1c90080baf72cb1), // 1e-22
        (0xd3f6fc16ebca5e03, 0x971da05074da7bee), // 1e-21
        (0x88f4bb1ca6bcf584, 0xbce5086492111aea), // 1e-20
        (0x2b31e9e3d06c32e5, 0xec1e4a7db69561a5), // 1e-19
        (0x3aff322e62439fcf, 0x9392ee8e921d5d07), // 1e-18
        (0x09befeb9fad487c2, 0xb877aa3236a4b449), // 1e-17
        (0x4c2ebe687989a9b3, 0xe69594bec44de15b), // 1e-16
        (0x0f9d37014bf60a10, 0x901d7cf73ab0acd9), // 1e-15
        (0x538484c19ef38c94, 0xb424dc35095cd80f), // 1e-14
        (0x2865a5f206b06fb9, 0xe12e13424bb40e13), // 1e-13
        (0xf93f87b7442e45d3, 0x8cbccc096f5088cb), // 1e-12
        (0xf78f69a51539d748, 0xafebff0bcb24aafe), // 1e-11
        (0xb573440e5a884d1b, 0xdbe6fecebdedd5be), // 1e-10
        (0x31680a88f8953030, 0x89705f4136b4a597), // 1e-9
        (0xfdc20d2b36ba7c3d, 0xabcc77118461cefc), // 1e-8
        (0x3d32907604691b4c, 0xd6bf94d5e57a42bc), // 1e-7
        (0xa63f9a49c2c1b10f, 0x8637bd05af6c69b5), // 1e-6
        (0x0fcf80dc33721d53, 0xa7c5ac471b478423), // 1e-5
        (0xd3c36113404ea4a8, 0xd1b71758e219652b), // 1e-4
        (0x645a1cac083126e9, 0x83126e978d4fdf3b), // 1e-3
        (0x3d70a3d70a3d70a3, 0xa3d70a3d70a3d70a), // 1e-2
        (0xcccccccccccccccc, 0xcccccccccccccccc), // 1e-1
        (0x0000000000000000, 0x8000000000000000), // 1e0
        (0x0000000000000000, 0xa000000000000000), // 1e1
        (0x0000000000000000, 0xc800000000000000), // 1e2
        (0x0000000000000000, 0xfa00000000000000), // 1e3
        (0x0000000000000000, 0x9c40000000000000), // 1e4
        (0x0000000000000000, 0xc350000000000000), // 1e5
        (0x0000000000000000, 0xf424000000000000), // 1e6
        (0x0000000000000000, 0x9896800000000000), // 1e7
        (0x0000000000000000, 0xbebc200000000000), // 1e8
        (0x0000000000000000, 0xee6b280000000000), // 1e9
        (0x0000000000000000, 0x9502f90000000000), // 1e10
        (0x0000000000000000, 0xba43b74000000000), // 1e11
        (0x0000000000000000, 0xe8d4a51000000000), // 1e12
        (0x0000000000000000, 0x9184e72a00000000), // 1e13
        (0x0000000000000000, 0xb5e620f480000000), // 1e14
        (0x0000000000000000, 0xe35fa931a0000000), // 1e15
        (0x0000000000000000, 0x8e1bc9bf04000000), // 1e16
        (0x0000000000000000, 0xb1a2bc2ec5000000), // 1e17
        (0x0000000000000000, 0xde0b6b3a76400000), // 1e18
        (0x0000000000000000, 0x8ac7230489e80000), // 1e19
        (0x0000000000000000, 0xad78ebc5ac620000), // 1e20
        (0x0000000000000000, 0xd8d726b7177a8000), // 1e21
        (0x0000000000000000, 0x878678326eac9000), // 1e22
        (0x0000000000000000, 0xa968163f0a57b400), // 1e23
        (0x0000000000000000, 0xd3c21bcecceda100), // 1e24
        (0x0000000000000000, 0x84595161401484a0), // 1e25
        (0x0000000000000000, 0xa56fa5b99019a5c8), // 1e26
        (0x0000000000000000, 0xcecb8f27f4200f3a), // 1e27
        (0x4000000000000000, 0x813f3978f8940984), // 1e28
        (0x5000000000000000, 0xa18f07d736b90be5), // 1e29
        (0xa400000000000000, 0xc9f2c9cd04674ede), // 1e30
        (0x4d00000000000000, 0xfc6f7c4045812296), // 1e31
        (0xf020000000000000, 0x9dc5ada82b70b59d), // 1e32
        (0x6c28000000000000, 0xc5371912364ce305), // 1e33
        (0xc732000000000000, 0xf684df56c3e01bc6), // 1e34
        (0x3c7f400000000000, 0x9a130b963a6c115c), // 1e35
        (0x4b9f100000000000, 0xc097ce7bc90715b3), // 1e36
        (0x1e86d40000000000, 0xf0bdc21abb48db20), // 1e37
        (0x1314448000000000, 0x96769950b50d88f4), // 1e38
        (0x17d955a000000000, 0xbc143fa4e250eb31), // 1e39
        (0x5dcfab0800000000, 0xeb194f8e1ae525fd), // 1e40
        (0x5aa1cae500000000, 0x92efd1b8d0cf37be), // 1e41
        (0xf14a3d9e40000000, 0xb7abc627050305ad), // 1e42
        (0x6d9ccd05d0000000, 0xe596b7b0c643c719), // 1e43
        (0xe4820023a2000000, 0x8f7e32ce7bea5c6f), // 1e44
        (0xdda2802c8a800000, 0xb35dbf821ae4f38b), // 1e45
        (0xd50b2037ad200000, 0xe0352f62a19e306e), // 1e46
        (0x4526f422cc340000, 0x8c213d9da502de45), // 1e47
        (0x9670b12b7f410000, 0xaf298d050e4395d6), // 1e48
        (0x3c0cdd765f114000, 0xdaf3f04651d47b4c), // 1e49
        (0xa5880a69fb6ac800, 0x88d8762bf324cd0f), // 1e50
        (0x8eea0d047a457a00, 0xab0e93b6efee0053), // 1e51
        (0x72a4904598d6d880, 0xd5d238a4abe98068), // 1e52
        (0x47a6da2b7f864750, 0x85a36366eb71f041), // 1e53
        (0x999090b65f67d924, 0xa70c3c40a64e6c51), // 1e54
        (0xfff4b4e3f741cf6d, 0xd0cf4b50cfe20765), // 1e55
        (0xbff8f10e7a8921a4, 0x82818f1281ed449f), // 1e56
        (0xaff72d52192b6a0d, 0xa321f2d7226895c7), // 1e57
        (0x9bf4f8a69f764490, 0xcbea6f8ceb02bb39), // 1e58
        (0x02f236d04753d5b4, 0xfee50b7025c36a08), // 1e59
        (0x01d762422c946590, 0x9f4f2726179a2245), // 1e60
        (0x424d3ad2b7b97ef5, 0xc722f0ef9d80aad6), // 1e61
        (0xd2e0898765a7deb2, 0xf8ebad2b84e0d58b), // 1e62
        (0x63cc55f49f88eb2f, 0x9b934c3b330c8577), // 1e63
        (0x3cbf6b71c76b25fb, 0xc2781f49ffcfa6d5), // 1e64
        (0x8bef464e3945ef7a, 0xf316271c7fc3908a), // 1e65
        (0x97758bf0e3cbb5ac, 0x97edd871cfda3a56), // 1e66
        (0x3d52eeed1cbea317, 0xbde94e8e43d0c8ec), // 1e67
        (0x4ca7aaa863ee4bdd, 0xed63a231d4c4fb27), // 1e68
        (0x8fe8caa93e74ef6a, 0x945e455f24fb1cf8), // 1e69
        (0xb3e2fd538e122b44, 0xb975d6b6ee39e436), // 1e70
        (0x60dbbca87196b616, 0xe7d34c64a9c85d44), // 1e71
        (0xbc8955e946fe31cd, 0x90e40fbeea1d3a4a), // 1e72
        (0x6babab6398bdbe41, 0xb51d13aea4a488dd), // 1e73
        (0xc696963c7eed2dd1, 0xe264589a4dcdab14), // 1e74
        (0xfc1e1de5cf543ca2, 0x8d7eb76070a08aec), // 1e75
        (0x3b25a55f43294bcb, 0xb0de65388cc8ada8), // 1e76
        (0x49ef0eb713f39ebe, 0xdd15fe86affad912), // 1e77
        (0x6e3569326c784337, 0x8a2dbf142dfcc7ab), // 1e78
        (0x49c2c37f07965404, 0xacb92ed9397bf996), // 1e79
        (0xdc33745ec97be906, 0xd7e77a8f87daf7fb), // 1e80
        (0x69a028bb3ded71a3, 0x86f0ac99b4e8dafd), // 1e81
        (0xc40832ea0d68ce0c, 0xa8acd7c0222311bc), // 1e82
        (0xf50a3fa490c30190, 0xd2d80db02aabd62b), // 1e83
        (0x792667c6da79e0fa, 0x83c7088e1aab65db), // 1e84
        (0x577001b891185938, 0xa4b8cab1a1563f52), // 1e85
        (0xed4c0226b55e6f86, 0xcde6fd5e09abcf26), // 1e86
        (0x544f8158315b05b4, 0x80b05e5ac60b6178), // 1e87
        (0x696361ae3db1c721, 0xa0dc75f1778e39d6), // 1e88
        (0x03bc3a19cd1e38e9, 0xc913936dd571c84c), // 1e89
        (0x04ab48a04065c723, 0xfb5878494ace3a5f), // 1e90
        (0x62eb0d64283f9c76, 0x9d174b2dcec0e47b), // 1e91
        (0x3ba5d0bd324f8394, 0xc45d1df942711d9a), // 1e92
        (0xca8f44ec7ee36479, 0xf5746577930d6500), // 1e93
        (0x7e998b13cf4e1ecb, 0x9968bf6abbe85f20), // 1e94
        (0x9e3fedd8c321a67e, 0xbfc2ef456ae276e8), // 1e95
        (0xc5cfe94ef3ea101e, 0xefb3ab16c59b14a2), // 1e96
        (0xbba1f1d158724a12, 0x95d04aee3b80ece5), // 1e97
        (0x2a8a6e45ae8edc97, 0xbb445da9ca61281f), // 1e98
        (0xf52d09d71a3293bd, 0xea1575143cf97226), // 1e99
        (0x593c2626705f9c56, 0x924d692ca61be758), // 1e100
        (0x6f8b2fb00c77836c, 0xb6e0c377cfa2e12e), // 1e101
        (0x0b6dfb9c0f956447, 0xe498f455c38b997a), // 1e102
        (0x4724bd4189bd5eac, 0x8edf98b59a373fec), // 1e103
        (0x58edec91ec2cb657, 0xb2977ee300c50fe7), // 1e104
        (0x2f2967b66737e3ed, 0xdf3d5e9bc0f653e1), // 1e105
        (0xbd79e0d20082ee74, 0x8b865b215899f46c), // 1e106
        (0xecd8590680a3aa11, 0xae67f1e9aec07187), // 1e107
        (0xe80e6f4820cc9495, 0xda01ee641a708de9), // 1e108
        (0x3109058d147fdcdd, 0x884134fe908658b2), // 1e109
        (0xbd4b46f0599fd415, 0xaa51823e34a7eede), // 1e110
        (0x6c9e18ac7007c91a, 0xd4e5e2cdc1d1ea96), // 1e111
        (0x03e2cf6bc604ddb0, 0x850fadc09923329e), // 1e112
        (0x84db8346b786151c, 0xa6539930bf6bff45), // 1e113
        (0xe612641865679a63, 0xcfe87f7cef46ff16), // 1e114
        (0x4fcb7e8f3f60c07e, 0x81f14fae158c5f6e), // 1e115
        (0xe3be5e330f38f09d, 0xa26da3999aef7749), // 1e116
        (0x5cadf5bfd3072cc5, 0xcb090c8001ab551c), // 1e117
        (0x73d9732fc7c8f7f6, 0xfdcb4fa002162a63), // 1e118
        (0x2867e7fddcdd9afa, 0x9e9f11c4014dda7e), // 1e119
        (0xb281e1fd541501b8, 0xc646d63501a1511d), // 1e120
        (0x1f225a7ca91a4226, 0xf7d88bc24209a565), // 1e121
        (0x3375788de9b06958, 0x9ae757596946075f), // 1e122
        (0x0052d6b1641c83ae, 0xc1a12d2fc3978937), // 1e123
        (0xc0678c5dbd23a49a, 0xf209787bb47d6b84), // 1e124
        (0xf840b7ba963646e0, 0x9745eb4d50ce6332), // 1e125
        (0xb650e5a93bc3d898, 0xbd176620a501fbff), // 1e126
        (0xa3e51f138ab4cebe, 0xec5d3fa8ce427aff), // 1e127
        (0xc66f336c36b10137, 0x93ba47c980e98cdf), // 1e128
        (0xb80b0047445d4184, 0xb8a8d9bbe123f017), // 1e129
        (0xa60dc059157491e5, 0xe6d3102ad96cec1d), // 1e130
        (0x87c89837ad68db2f, 0x9043ea1ac7e41392), // 1e131
        (0x29babe4598c311fb, 0xb454e4a179dd1877), // 1e132
        (0xf4296dd6fef3d67a, 0xe16a1dc9d8545e94), // 1e133
        (0x1899e4a65f58660c, 0x8ce2529e2734bb1d), // 1e134
        (0x5ec05dcff72e7f8f, 0xb01ae745b101e9e4), // 1e135
        (0x76707543f4fa1f73, 0xdc21a1171d42645d), // 1e136
        (0x6a06494a791c53a8, 0x899504ae72497eba), // 1e137
        (0x0487db9d17636892, 0xabfa45da0edbde69), // 1e138
        (0x45a9d2845d3c42b6, 0xd6f8d7509292d603), // 1e139
        (0x0b8a2392ba45a9b2, 0x865b86925b9bc5c2), // 1e140
        (0x8e6cac7768d7141e, 0xa7f26836f282b732), // 1e141
        (0x3207d795430cd926, 0xd1ef0244af2364ff), // 1e142
        (0x7f44e6bd49e807b8, 0x8335616aed761f1f), // 1e143
        (0x5f16206c9c6209a6, 0xa402b9c5a8d3a6e7), // 1e144
        (0x36dba887c37a8c0f, 0xcd036837130890a1), // 1e145
        (0xc2494954da2c9789, 0x802221226be55a64), // 1e146
        (0xf2db9baa10b7bd6c, 0xa02aa96b06deb0fd), // 1e147
        (0x6f92829494e5acc7, 0xc83553c5c8965d3d), // 1e148
        (0xcb772339ba1f17f9, 0xfa42a8b73abbf48c), // 1e149
        (0xff2a760414536efb, 0x9c69a97284b578d7), // 1e150
        (0xfef5138519684aba, 0xc38413cf25e2d70d), // 1e151
        (0x7eb258665fc25d69, 0xf46518c2ef5b8cd1), // 1e152
        (0xef2f773ffbd97a61, 0x98bf2f79d5993802), // 1e153
        (0xaafb550ffacfd8fa, 0xbeeefb584aff8603), // 1e154
        (0x95ba2a53f983cf38, 0xeeaaba2e5dbf6784), // 1e155
        (0xdd945a747bf26183, 0x952ab45cfa97a0b2), // 1e156
        (0x94f971119aeef9e4, 0xba756174393d88df), // 1e157
        (0x7a37cd5601aab85d, 0xe912b9d1478ceb17), // 1e158
        (0xac62e055c10ab33a, 0x91abb422ccb812ee), // 1e159
        (0x577b986b314d6009, 0xb616a12b7fe617aa), // 1e160
        (0xed5a7e85fda0b80b, 0xe39c49765fdf9d94), // 1e161
        (0x14588f13be847307, 0x8e41ade9fbebc27d), // 1e162
        (0x596eb2d8ae258fc8, 0xb1d219647ae6b31c), // 1e163
        (0x6fca5f8ed9aef3bb, 0xde469fbd99a05fe3), // 1e164
        (0x25de7bb9480d5854, 0x8aec23d680043bee), // 1e165
        (0xaf561aa79a10ae6a, 0xada72ccc20054ae9), // 1e166
        (0x1b2ba1518094da04, 0xd910f7ff28069da4), // 1e167
        (0x90fb44d2f05d0842, 0x87aa9aff79042286), // 1e168
        (0x353a1607ac744a53, 0xa99541bf57452b28), // 1e169
        (0x42889b8997915ce8, 0xd3fa922f2d1675f2), // 1e170
        (0x69956135febada11, 0x847c9b5d7c2e09b7), // 1e171
        (0x43fab9837e699095, 0xa59bc234db398c25), // 1e172
        (0x94f967e45e03f4bb, 0xcf02b2c21207ef2e), // 1e173
        (0x1d1be0eebac278f5, 0x8161afb94b44f57d), // 1e174
        (0x6462d92a69731732, 0xa1ba1ba79e1632dc), // 1e175
        (0x7d7b8f7503cfdcfe, 0xca28a291859bbf93), // 1e176
        (0x5cda735244c3d43e, 0xfcb2cb35e702af78), // 1e177
        (0x3a0888136afa64a7, 0x9defbf01b061adab), // 1e178
        (0x088aaa1845b8fdd0, 0xc56baec21c7a1916), // 1e179
        (0x8aad549e57273d45, 0xf6c69a72a3989f5b), // 1e180
        (0x36ac54e2f678864b, 0x9a3c2087a63f6399), // 1e181
        (0x84576a1bb416a7dd, 0xc0cb28a98fcf3c7f), // 1e182
        (0x656d44a2a11c51d5, 0xf0fdf2d3f3c30b9f), // 1e183
        (0x9f644ae5a4b1b325, 0x969eb7c47859e743), // 1e184
        (0x873d5d9f0dde1fee, 0xbc4665b596706114), // 1e185
        (0xa90cb506d155a7ea, 0xeb57ff22fc0c7959), // 1e186
        (0x09a7f12442d588f2, 0x9316ff75dd87cbd8), // 1e187
        (0x0c11ed6d538aeb2f, 0xb7dcbf5354e9bece), // 1e188
        (0x8f1668c8a86da5fa, 0xe5d3ef282a242e81), // 1e189
        (0xf96e017d694487bc, 0x8fa475791a569d10), // 1e190
        (0x37c981dcc395a9ac, 0xb38d92d760ec4455), // 1e191
        (0x85bbe253f47b1417, 0xe070f78d3927556a), // 1e192
        (0x93956d7478ccec8e, 0x8c469ab843b89562), // 1e193
        (0x387ac8d1970027b2, 0xaf58416654a6babb), // 1e194
        (0x06997b05fcc0319e, 0xdb2e51bfe9d0696a), // 1e195
        (0x441fece3bdf81f03, 0x88fcf317f22241e2), // 1e196
        (0xd527e81cad7626c3, 0xab3c2fddeeaad25a), // 1e197
        (0x8a71e223d8d3b074, 0xd60b3bd56a5586f1), // 1e198
        (0xf6872d5667844e49, 0x85c7056562757456), // 1e199
        (0xb428f8ac016561db, 0xa738c6bebb12d16c), // 1e200
        (0xe13336d701beba52, 0xd106f86e69d785c7), // 1e201
        (0xecc0024661173473, 0x82a45b450226b39c), // 1e202
        (0x27f002d7f95d0190, 0xa34d721642b06084), // 1e203
        (0x31ec038df7b441f4, 0xcc20ce9bd35c78a5), // 1e204
        (0x7e67047175a15271, 0xff290242c83396ce), // 1e205
        (0x0f0062c6e984d386, 0x9f79a169bd203e41), // 1e206
        (0x52c07b78a3e60868, 0xc75809c42c684dd1), // 1e207
        (0xa7709a56ccdf8a82, 0xf92e0c3537826145), // 1e208
        (0x88a66076400bb691, 0x9bbcc7a142b17ccb), // 1e209
        (0x6acff893d00ea435, 0xc2abf989935ddbfe), // 1e210
        (0x0583f6b8c4124d43, 0xf356f7ebf83552fe), // 1e211
        (0xc3727a337a8b704a, 0x98165af37b2153de), // 1e212
        (0x744f18c0592e4c5c, 0xbe1bf1b059e9a8d6), // 1e213
        (0x1162def06f79df73, 0xeda2ee1c7064130c), // 1e214
        (0x8addcb5645ac2ba8, 0x9485d4d1c63e8be7), // 1e215
        (0x6d953e2bd7173692, 0xb9a74a0637ce2ee1), // 1e216
        (0xc8fa8db6ccdd0437, 0xe8111c87c5c1ba99), // 1e217
        (0x1d9c9892400a22a2, 0x910ab1d4db9914a0), // 1e218
        (0x2503beb6d00cab4b, 0xb54d5e4a127f59c8), // 1e219
        (0x2e44ae64840fd61d, 0xe2a0b5dc971f303a), // 1e220
        (0x5ceaecfed289e5d2, 0x8da471a9de737e24), // 1e221
        (0x7425a83e872c5f47, 0xb10d8e1456105dad), // 1e222
        (0xd12f124e28f77719, 0xdd50f1996b947518), // 1e223
        (0x82bd6b70d99aaa6f, 0x8a5296ffe33cc92f), // 1e224
        (0x636cc64d1001550b, 0xace73cbfdc0bfb7b), // 1e225
        (0x3c47f7e05401aa4e, 0xd8210befd30efa5a), // 1e226
        (0x65acfaec34810a71, 0x8714a775e3e95c78), // 1e227
        (0x7f1839a741a14d0d, 0xa8d9d1535ce3b396), // 1e228
        (0x1ede48111209a050, 0xd31045a8341ca07c), // 1e229
        (0x934aed0aab460432, 0x83ea2b892091e44d), // 1e230
        (0xf81da84d5617853f, 0xa4e4b66b68b65d60), // 1e231
        (0x36251260ab9d668e, 0xce1de40642e3f4b9), // 1e232
        (0xc1d72b7c6b426019, 0x80d2ae83e9ce78f3), // 1e233
        (0xb24cf65b8612f81f, 0xa1075a24e4421730), // 1e234
        (0xdee033f26797b627, 0xc94930ae1d529cfc), // 1e235
        (0x169840ef017da3b1, 0xfb9b7cd9a4a7443c), // 1e236
        (0x8e1f289560ee864e, 0x9d412e0806e88aa5), // 1e237
        (0xf1a6f2bab92a27e2, 0xc491798a08a2ad4e), // 1e238
        (0xae10af696774b1db, 0xf5b5d7ec8acb58a2), // 1e239
        (0xacca6da1e0a8ef29, 0x9991a6f3d6bf1765), // 1e240
        (0x17fd090a58d32af3, 0xbff610b0cc6edd3f), // 1e241
        (0xddfc4b4cef07f5b0, 0xeff394dcff8a948e), // 1e242
        (0x4abdaf101564f98e, 0x95f83d0a1fb69cd9), // 1e243
        (0x9d6d1ad41abe37f1, 0xbb764c4ca7a4440f), // 1e244
        (0x84c86189216dc5ed, 0xea53df5fd18d5513), // 1e245
        (0x32fd3cf5b4e49bb4, 0x92746b9be2f8552c), // 1e246
        (0x3fbc8c33221dc2a1, 0xb7118682dbb66a77), // 1e247
        (0x0fabaf3feaa5334a, 0xe4d5e82392a40515), // 1e248
        (0x29cb4d87f2a7400e, 0x8f05b1163ba6832d), // 1e249
        (0x743e20e9ef511012, 0xb2c71d5bca9023f8), // 1e250
        (0x914da9246b255416, 0xdf78e4b2bd342cf6), // 1e251
        (0x1ad089b6c2f7548e, 0x8bab8eefb6409c1a), // 1e252
        (0xa184ac2473b529b1, 0xae9672aba3d0c320), // 1e253
        (0xc9e5d72d90a2741e, 0xda3c0f568cc4f3e8), // 1e254
        (0x7e2fa67c7a658892, 0x8865899617fb1871), // 1e255
        (0xddbb901b98feeab7, 0xaa7eebfb9df9de8d), // 1e256
        (0x552a74227f3ea565, 0xd51ea6fa85785631), // 1e257
        (0xd53a88958f87275f, 0x8533285c936b35de), // 1e258
        (0x8a892abaf368f137, 0xa67ff273b8460356), // 1e259
        (0x2d2b7569b0432d85, 0xd01fef10a657842c), // 1e260
        (0x9c3b29620e29fc73, 0x8213f56a67f6b29b), // 1e261
        (0x8349f3ba91b47b8f, 0xa298f2c501f45f42), // 1e262
        (0x241c70a936219a73, 0xcb3f2f7642717713), // 1e263
        (0xed238cd383aa0110, 0xfe0efb53d30dd4d7), // 1e264
        (0xf4363804324a40aa, 0x9ec95d1463e8a506), // 1e265
        (0xb143c6053edcd0d5, 0xc67bb4597ce2ce48), // 1e266
        (0xdd94b7868e94050a, 0xf81aa16fdc1b81da), // 1e267
        (0xca7cf2b4191c8326, 0x9b10a4e5e9913128), // 1e268
        (0xfd1c2f611f63a3f0, 0xc1d4ce1f63f57d72), // 1e269
        (0xbc633b39673c8cec, 0xf24a01a73cf2dccf), // 1e270
        (0xd5be0503e085d813, 0x976e41088617ca01), // 1e271
        (0x4b2d8644d8a74e18, 0xbd49d14aa79dbc82), // 1e272
        (0xddf8e7d60ed1219e, 0xec9c459d51852ba2), // 1e273
        (0xcabb90e5c942b503, 0x93e1ab8252f33b45), // 1e274
        (0x3d6a751f3b936243, 0xb8da1662e7b00a17), // 1e275
        (0x0cc512670a783ad4, 0xe7109bfba19c0c9d), // 1e276
        (0x27fb2b80668b24c5, 0x906a617d450187e2), // 1e277
        (0xb1f9f660802dedf6, 0xb484f9dc9641e9da), // 1e278
        (0x5e7873f8a0396973, 0xe1a63853bbd26451), // 1e279
        (0xdb0b487b6423e1e8, 0x8d07e33455637eb2), // 1e280
        (0x91ce1a9a3d2cda62, 0xb049dc016abc5e5f), // 1e281
        (0x7641a140cc7810fb, 0xdc5c5301c56b75f7), // 1e282
        (0xa9e904c87fcb0a9d, 0x89b9b3e11b6329ba), // 1e283
        (0x546345fa9fbdcd44, 0xac2820d9623bf429), // 1e284
        (0xa97c177947ad4095, 0xd732290fbacaf133), // 1e285
        (0x49ed8eabcccc485d, 0x867f59a9d4bed6c0), // 1e286
        (0x5c68f256bfff5a74, 0xa81f301449ee8c70), // 1e287
        (0x73832eec6fff3111, 0xd226fc195c6a2f8c), // 1e288
        (0xc831fd53c5ff7eab, 0x83585d8fd9c25db7), // 1e289
        (0xba3e7ca8b77f5e55, 0xa42e74f3d032f525), // 1e290
        (0x28ce1bd2e55f35eb, 0xcd3a1230c43fb26f), // 1e291
        (0x7980d163cf5b81b3, 0x80444b5e7aa7cf85), // 1e292
        (0xd7e105bcc332621f, 0xa0555e361951c366), // 1e293
        (0x8dd9472bf3fefaa7, 0xc86ab5c39fa63440), // 1e294
        (0xb14f98f6f0feb951, 0xfa856334878fc150), // 1e295
        (0x6ed1bf9a569f33d3, 0x9c935e00d4b9d8d2), // 1e296
        (0x0a862f80ec4700c8, 0xc3b8358109e84f07), // 1e297
        (0xcd27bb612758c0fa, 0xf4a642e14c6262c8), // 1e298
        (0x8038d51cb897789c, 0x98e7e9cccfbd7dbd), // 1e299
        (0xe0470a63e6bd56c3, 0xbf21e44003acdd2c), // 1e300
        (0x1858ccfce06cac74, 0xeeea5d5004981478), // 1e301
        (0x0f37801e0c43ebc8, 0x95527a5202df0ccb), // 1e302
        (0xd30560258f54e6ba, 0xbaa718e68396cffd), // 1e303
        (0x47c6b82ef32a2069, 0xe950df20247c83fd), // 1e304
        (0x4cdc331d57fa5441, 0x91d28b7416cdd27e), // 1e305
        (0xe0133fe4adf8e952, 0xb6472e511c81471d), // 1e306
        (0x58180fddd97723a6, 0xe3d8f9e563a198e5), // 1e307
        (0x570f09eaa7ea7648, 0x8e679c2f5e44ff8f), // 1e308
        (0x2cd2cc6551e513da, 0xb201833b35d63f73), // 1e309
        (0xf8077f7ea65e58d1, 0xde81e40a034bcf4f), // 1e310
        (0xfb04afaf27faf782, 0x8b112e86420f6191), // 1e311
        (0x79c5db9af1f9b563, 0xadd57a27d29339f6), // 1e312
        (0x18375281ae7822bc, 0xd94ad8b1c7380874), // 1e313
        (0x8f2293910d0b15b5, 0x87cec76f1c830548), // 1e314
        (0xb2eb3875504ddb22, 0xa9c2794ae3a3c69a), // 1e315
        (0x5fa60692a46151eb, 0xd433179d9c8cb841), // 1e316
        (0xdbc7c41ba6bcd333, 0x849feec281d7f328), // 1e317
        (0x12b9b522906c0800, 0xa5c7ea73224deff3), // 1e318
        (0xd768226b34870a00, 0xcf39e50feae16bef), // 1e319
        (0xe6a1158300d46640, 0x81842f29f2cce375), // 1e320
        (0x60495ae3c1097fd0, 0xa1e53af46f801c53), // 1e321
        (0x385bb19cb14bdfc4, 0xca5e89b18b602368), // 1e322
        (0x46729e03dd9ed7b5, 0xfcf62c1dee382c42), // 1e323
        (0x6c07a2c26a8346d1, 0x9e19db92b4e31ba9), // 1e324
        (0xc7098b7305241885, 0xc5a05277621be293), // 1e325
        (0xb8cbee4fc66d1ea7, 0xf70867153aa2db38), // 1e326
        (0x737f74f1dc043328, 0x9a65406d44a5c903), // 1e327
        (0x505f522e53053ff2, 0xc0fe908895cf3b44), // 1e328
        (0x647726b9e7c68fef, 0xf13e34aabb430a15), // 1e329
        (0x5eca783430dc19f5, 0x96c6e0eab509e64d), // 1e330
        (0xb67d16413d132072, 0xbc789925624c5fe0), // 1e331
        (0xe41c5bd18c57e88f, 0xeb96bf6ebadf77d8), // 1e332
        (0x8e91b962f7b6f159, 0x933e37a534cbaae7), // 1e333
        (0x723627bbb5a4adb0, 0xb80dc58e81fe95a1), // 1e334
        (0xcec3b1aaa30dd91c, 0xe61136f2227e3b09), // 1e335
        (0x213a4f0aa5e8a7b1, 0x8fcac257558ee4e6), // 1e336
        (0xa988e2cd4f62d19d, 0xb3bd72ed2af29e1f), // 1e337
        (0x93eb1b80a33b8605, 0xe0accfa875af45a7), // 1e338
        (0xbc72f130660533c3, 0x8c6c01c9498d8b88), // 1e339
        (0xeb8fad7c7f8680b4, 0xaf87023b9bf0ee6a), // 1e340
        (0xa67398db9f6820e1, 0xdb68c2ca82ed2a05), // 1e341
        (0x88083f8943a1148c, 0x892179be91d43a43), // 1e342
        (0x6a0a4f6b948959b0, 0xab69d82e364948d4), // 1e343
        (0x848ce34679abb01c, 0xd6444e39c3db9b09), // 1e344
        (0xf2d80e0c0c0b4e11, 0x85eab0e41a6940e5), // 1e345
        (0x6f8e118f0f0e2195, 0xa7655d1d2103911f), // 1e346
        (0x4b7195f2d2d1a9fb, 0xd13eb46469447567), // 1e347
    ];

    fn mul(x: u64, y: u64) -> (u64, u64) {
        let z = (x as u128) * (y as u128);
        ((z >> 64) as u64, z as u64)
    }

    if man == 0 || exp10 < POW10_MIN_EXP10 {
        if neg {
            return Some(-0.0);
        } else {
            return Some(0.0);
        }
    }

    if exp10 > POW10_MAX_EXP10 {
        if neg {
            return Some(f64::NEG_INFINITY);
        } else {
            return Some(f64::INFINITY);
        }
    }

    // Fast path for small values
    if (man >> 52) == 0 {
        const SMALL_POW10: [f64; 23] = [
            1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
            1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
        ];

        let mut f = man as f64;
        if neg {
            f = -f;
        }
        if exp10 == 0 {
            return Some(f);
        }
        if 0 < exp10 && exp10 <= 15 + 22 {
            let exp10 = if exp10 > 22 {
                f *= SMALL_POW10[(exp10 - 22) as usize];
                22
            } else {
                exp10
            };
            if -1e15 <= f && f <= 1e15 {
                return Some(f * SMALL_POW10[exp10 as usize]);
            }
        } else if -22 <= exp10 && exp10 < 0 {
            return Some(f / SMALL_POW10[-exp10 as usize]);
        }
    }

    let clz = man.leading_zeros();
    man <<= clz;
    let mut ret_exp2 = (((217706 * exp10) >> 16) + 64 + 1023) as u64 - clz as u64;

    let (pow_of_10_lo, pow_of_10_hi) = DETAILED_POWERS_OF_TEN[(exp10 - POW10_MIN_EXP10) as usize];

    let (mut x_hi, mut x_lo) = mul(man, pow_of_10_hi);

    if x_hi & 0x1ff == 0x1ff && x_lo.wrapping_add(man) < man {
        let (y_hi, y_lo) = mul(man, pow_of_10_lo);
        let (mut merged_hi, merged_lo) = (x_hi, x_lo.wrapping_add(y_hi));
        if merged_lo < x_lo {
            merged_hi += 1;
        }
        if merged_hi & 0x1ff == 0x1ff
            && merged_lo.wrapping_add(1) == 0
            && y_lo.wrapping_add(man) < man
        {
            return None;
        }
        (x_hi, x_lo) = (merged_hi, merged_lo);
    }

    let msb = x_hi >> 63;
    let mut ret_man = x_hi >> (msb + 9);
    ret_exp2 -= 1 ^ msb;

    if x_lo == 0 && x_hi & 0x1ff == 0 && ret_man & 3 == 1 {
        return None; // Ambiguous
    }

    ret_man += ret_man & 1;
    ret_man >>= 1;
    if (ret_man >> 53) > 0 {
        ret_man >>= 1;
        ret_exp2 += 1;
    }

    if ret_exp2 >= 0x7ff {
        return None; // Subnormal
    }

    let mut ret = ret_exp2 << 52 | ret_man & 0x000fffffffffffff;
    if neg {
        ret |= 0x8000000000000000;
    }
    Some(f64::from_bits(ret))
}

/// JSON parser to parse UTF-8 string into `JsonValue` value.
///
/// Basically you don't need to use this struct directly thanks to `FromStr` trait implementation.
///
/// ```
/// use tinyjson::{JsonParser, JsonValue};
///
/// let mut parser = JsonParser::new("[1, 2, 3]".chars());
/// let array = parser.parse().unwrap();
///
/// // Equivalent to the above code using `FromStr`
/// let array: JsonValue = "[1, 2, 3]".parse().unwrap();
/// ```
pub struct JsonParser<I>
where
    I: Iterator<Item = char>,
{
    chars: Peekable<I>,
    line: usize,
    col: usize,
}

impl<I: Iterator<Item = char>> JsonParser<I> {
    /// Create a new parser instance from an iterator which iterates characters. The iterator is usually built from
    /// `str::chars` for parsing `str` or `String` values.
    pub fn new(it: I) -> Self {
        JsonParser {
            chars: it.peekable(),
            line: 1,
            col: 0,
        }
    }

    fn err<T>(&self, msg: String) -> Result<T, JsonParseError> {
        Err(JsonParseError::new(msg, self.line, self.col))
    }

    fn unexpected_eof(&self) -> Result<char, JsonParseError> {
        Err(JsonParseError::new(
            String::from("Unexpected EOF"),
            self.line,
            self.col,
        ))
    }

    fn next_pos(&mut self, c: char) {
        if c == '\n' {
            self.col = 0;
            self.line += 1;
        } else {
            self.col += 1;
        }
    }

    fn peek(&mut self) -> Result<char, JsonParseError> {
        while let Some(c) = self.chars.peek().copied() {
            if !is_whitespace(c) {
                return Ok(c);
            }
            self.next_pos(c);
            self.chars.next().unwrap();
        }
        self.unexpected_eof()
    }

    fn next(&mut self) -> Option<char> {
        while let Some(c) = self.chars.next() {
            self.next_pos(c);
            if !is_whitespace(c) {
                return Some(c);
            }
        }
        None
    }

    fn consume(&mut self) -> Result<char, JsonParseError> {
        if let Some(c) = self.next() {
            Ok(c)
        } else {
            self.unexpected_eof()
        }
    }

    fn consume_no_skip(&mut self) -> Result<char, JsonParseError> {
        if let Some(c) = self.chars.next() {
            self.next_pos(c);
            Ok(c)
        } else {
            self.unexpected_eof()
        }
    }

    fn parse_object(&mut self) -> JsonParseResult {
        if self.consume()? != '{' {
            return self.err(String::from("Object must starts with '{'"));
        }

        if self.peek()? == '}' {
            self.consume().unwrap();
            return Ok(JsonValue::Object(HashMap::new()));
        }

        let mut m = HashMap::new();
        loop {
            let key = match self.parse_any()? {
                JsonValue::String(s) => s,
                v => return self.err(format!("Key of object must be string but found {:?}", v)),
            };

            let c = self.consume()?;
            if c != ':' {
                return self.err(format!(
                    "':' is expected after key of object but actually found '{}'",
                    c
                ));
            }

            m.insert(key, self.parse_any()?);

            match self.consume()? {
                ',' => {}
                '}' => return Ok(JsonValue::Object(m)),
                c => {
                    return self.err(format!(
                        "',' or '}}' is expected for object but actually found '{}'",
                        c.escape_debug(),
                    ))
                }
            }
        }
    }

    fn parse_array(&mut self) -> JsonParseResult {
        if self.consume()? != '[' {
            return self.err(String::from("Array must starts with '['"));
        }

        if self.peek()? == ']' {
            self.consume().unwrap();
            return Ok(JsonValue::Array(vec![]));
        }

        let mut v = vec![self.parse_any()?];
        loop {
            match self.consume()? {
                ',' => {}
                ']' => return Ok(JsonValue::Array(v)),
                c => {
                    return self.err(format!(
                        "',' or ']' is expected for array but actually found '{}'",
                        c
                    ))
                }
            }

            v.push(self.parse_any()?); // Next element
        }
    }

    fn push_utf16(&self, s: &mut String, utf16: &mut Vec<u16>) -> Result<(), JsonParseError> {
        if utf16.is_empty() {
            return Ok(());
        }

        match String::from_utf16(utf16) {
            Ok(utf8) => s.push_str(&utf8),
            Err(err) => return self.err(format!("Invalid UTF-16 sequence {:?}: {}", &utf16, err)),
        }
        utf16.clear();
        Ok(())
    }

    fn parse_string(&mut self) -> JsonParseResult {
        if self.consume()? != '"' {
            return self.err(String::from("String must starts with double quote"));
        }

        let mut utf16 = Vec::new(); // Buffer for parsing \uXXXX UTF-16 characters
        let mut s = String::new();
        loop {
            let c = match self.consume_no_skip()? {
                '\\' => match self.consume_no_skip()? {
                    '\\' => '\\',
                    '/' => '/',
                    '"' => '"',
                    'b' => '\u{0008}',
                    'f' => '\u{000c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'u' => {
                        let mut u = 0u16;
                        for _ in 0..4 {
                            let c = self.consume()?;
                            if let Some(h) = c.to_digit(16) {
                                u = u * 0x10 + h as u16;
                            } else {
                                return self.err(format!("Unicode character must be \\uXXXX (X is hex character) format but found character '{}'", c));
                            }
                        }
                        utf16.push(u);
                        // Additional \uXXXX character may follow. UTF-16 characters must be converted
                        // into UTF-8 string as sequence because surrogate pairs must be considered
                        // like "\uDBFF\uDFFF".
                        continue;
                    }
                    c => return self.err(format!("'\\{}' is invalid escaped character", c)),
                },
                '"' => {
                    self.push_utf16(&mut s, &mut utf16)?;
                    return Ok(JsonValue::String(s));
                }
                // Note: c.is_control() is not available here because JSON accepts 0x7f (DEL) in
                // string literals but 0x7f is control character.
                // Rough spec of JSON says string literal cannot contain control characters. But it
                // can actually contain 0x7f.
                c if (c as u32) < 0x20 => {
                    return self.err(format!(
                        "String cannot contain control character {}",
                        c.escape_debug(),
                    ));
                }
                c => c,
            };

            self.push_utf16(&mut s, &mut utf16)?;

            s.push(c);
        }
    }

    fn parse_constant(&mut self, s: &'static str) -> Option<JsonParseError> {
        for c in s.chars() {
            match self.consume_no_skip() {
                Ok(x) if x != c => {
                    return Some(JsonParseError::new(
                        format!("Unexpected character '{}' while parsing '{}'", c, s),
                        self.line,
                        self.col,
                    ));
                }
                Ok(_) => {}
                Err(e) => return Some(e),
            }
        }
        None
    }

    fn parse_null(&mut self) -> JsonParseResult {
        match self.parse_constant("null") {
            Some(err) => Err(err),
            None => Ok(JsonValue::Null),
        }
    }

    fn parse_true(&mut self) -> JsonParseResult {
        match self.parse_constant("true") {
            Some(err) => Err(err),
            None => Ok(JsonValue::Boolean(true)),
        }
    }

    fn parse_false(&mut self) -> JsonParseResult {
        match self.parse_constant("false") {
            Some(err) => Err(err),
            None => Ok(JsonValue::Boolean(false)),
        }
    }

    fn parse_number(&mut self) -> JsonParseResult {
        let neg = if let Some('-') = self.chars.peek() {
            self.consume_no_skip().unwrap();
            true
        } else {
            false
        };

        let mut trunc = false;
        let mut exponent: i32 = 0;
        let mut mantissa = match self.consume_no_skip()? {
            '0' => 0,
            d @ '1'..='9' => {
                let mut man = d.to_digit(10).unwrap() as u64;
                while let Some('0'..='9') = self.chars.peek() {
                    let d = self.consume_no_skip().unwrap();
                    if let Some(m) = man
                        .checked_mul(10)
                        .and_then(|m| m.checked_add(d.to_digit(10).unwrap() as u64))
                    {
                        man = m;
                    } else {
                        exponent = exponent.saturating_add(1);
                        trunc = true;
                    }
                }
                man
            }
            c => {
                return self.err(format!(
                    "expected '0'~'9' for integer part of number but got {}",
                    c
                ));
            }
        };

        if let Some('.') = self.chars.peek() {
            self.consume_no_skip().unwrap(); // Eat '.'

            match self.consume_no_skip()? {
                d @ '0'..='9' => {
                    if let Some(m) = mantissa
                        .checked_mul(10)
                        .and_then(|m| m.checked_add(d.to_digit(10).unwrap() as u64))
                    {
                        mantissa = m;
                        exponent = exponent.saturating_sub(1);
                    } else {
                        trunc = true;
                    }
                }
                c => {
                    return self.err(format!(
                        "at least one digit must follow after '.' but got {}",
                        c
                    ));
                }
            }

            while let Some('0'..='9') = self.chars.peek() {
                let d = self.consume_no_skip().unwrap();
                if let Some(m) = mantissa
                    .checked_mul(10)
                    .and_then(|m| m.checked_add(d.to_digit(10).unwrap() as u64))
                {
                    mantissa = m;
                    exponent = exponent.saturating_sub(1);
                }
            }
        }

        if let Some('e' | 'E') = self.chars.peek() {
            self.consume_no_skip().unwrap(); // Eat 'e' or 'E'

            let neg = match self.chars.peek() {
                Some('-') => {
                    self.consume_no_skip().unwrap();
                    true
                }
                Some('+') => {
                    self.consume_no_skip().unwrap();
                    false
                }
                _ => false,
            };

            let mut exp = match self.consume_no_skip()? {
                d @ '0'..='9' => d.to_digit(10).unwrap() as i32,
                c => {
                    return self.err(format!(
                        "at least one digit is necessary at exponent part of number but got {}",
                        c
                    ));
                }
            };

            while let Some('0'..='9') = self.chars.peek() {
                let d = self.consume_no_skip().unwrap();
                exp = exp
                    .saturating_mul(10)
                    .saturating_add(d.to_digit(10).unwrap() as i32);
            }

            if neg {
                exp = -exp;
            }
            exponent = exponent.saturating_add(exp);
        }

        if let Some(f) = eisel_lemire(mantissa, exponent, neg) {
            if !trunc {
                return Ok(JsonValue::Number(f));
            }
            if let Some(upper) = eisel_lemire(mantissa + 1, exponent, neg) {
                if upper == f {
                    return Ok(JsonValue::Number(f));
                }
            }
        }

        todo!(
            "slow path to parse float number: ambiguous half-way: {:?}",
            (mantissa, exponent)
        )
    }

    fn parse_any(&mut self) -> JsonParseResult {
        match self.peek()? {
            '0'..='9' | '-' => self.parse_number(),
            '"' => self.parse_string(),
            '[' => self.parse_array(),
            '{' => self.parse_object(),
            't' => self.parse_true(),
            'f' => self.parse_false(),
            'n' => self.parse_null(),
            c => self.err(format!("Invalid character: {}", c.escape_debug())),
        }
    }

    /// Run the parser to parse one JSON value.
    pub fn parse(&mut self) -> JsonParseResult {
        let v = self.parse_any()?;

        if let Some(c) = self.next() {
            return self.err(format!(
                "Expected EOF but got character '{}'",
                c.escape_debug(),
            ));
        }

        Ok(v)
    }
}

/// Parse given `str` object into `JsonValue` value. This is recommended way to parse strings into JSON value with
/// this library.
///
/// ```
/// use tinyjson::JsonValue;
///
/// let array: JsonValue = "[1, 2, 3]".parse().unwrap();
/// assert!(array.is_array());
/// ```
impl FromStr for JsonValue {
    type Err = JsonParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        JsonParser::new(s.chars()).parse()
    }
}
