# Map ownership and storage explicitly

- Status: accepted
- Created: 2026-10-31
- Updated: 2026-11-02

C++ pointer arithmetic, mutable visitors, and partially initialized storage
cannot be copied literally into safe Rust. Preserve their defined behavior with
explicit representations while retaining source algorithms.

- A bit reader borrows its bytes. Byte offsets replace pointers, including the
  source's end-minus-eight sentinel. Preserve its 64-bit refill buffer,
  zero-extension, deferred bounds failures, and explicit `close` checkpoint.
  Native Rust moves transfer the close obligation; no copying is provided.
  Debug destruction checks do not raise a second panic during Rust unwinding.
- `Status` maps to `Result<(), StatusCode>` and `StatusOr<T>` to
  `Result<T, StatusCode>`. Success is `Ok`; error codes retain upstream numeric
  values. Error diagnostics and crash-on-error instrumentation are not yet
  translated. Boolean control predicates remain booleans.
  In particular, `AllDefault` consumes a source Status as a branch condition:
  a failed read means false and field visitation continues. Returning early
  with `?` changes partial state and bit consumption; a differential regression
  covers this with an empty, truncated bundle.
- Field visitors mutate borrowed fields. Bundle encoding accepts a mutable
  bundle because upstream's const visitor casts away constness and may update
  `all_default`; do not clone complete bundles to hide that mutation.
- Serialized enums use raw `u32` wrappers implementing `FieldEnum`, so an
  invalid decoded value can be stored and then rejected in the same order as
  C++. Never create an invalid Rust enum discriminant. The templated Enum
  visitor helper is a free function to keep `Visitor` object-safe.
- ANS alias entries retain the eight-byte packed layout and original word
  lookup on little-endian targets. The unaligned word read is contained in
  the lookup with its layout, initialization, and borrow proof. Prefetch uses
  the source's Clang configuration: a T0 read hint on x86_64 with SSE2, disabled
  otherwise. Read top-level preprocessor guards as well as function bodies:
  Highway's SSE2 guard disables its apparent AArch64 builtin branch. Emitting
  an ARM hint would introduce an optimization absent from the reference.
- Entropy readers borrow their code tables for their entire lifetime and take
  a separate mutable bit-reader borrow for each operation. Aligned storage
  owns typed integer/alias slices; local proofs cover initialization, alignment,
  bounds, and exclusive mutable access. The LZ77 window starts initialized as
  required for safe Rust storage; defined decoder reads still follow the
  source's history/zero-distance rules. Keep all state and history counters
  32-bit with explicit wrapping, including the unhealthy-reader path for a
  wrapping decoded run length. Checkpoints restore entropy state and the same
  512 window entries; restoring the bit-reader position remains the caller's
  responsibility. Token bitfields use a private packed `u32` and accessors that
  retain truncation to the source's 31-bit context field. This does not yet
  establish a public C bitfield ABI.
- The inverse move-to-front transform initially translates the existing
  one-lane Highway scalar specialization. Its vector specializations remain
  separate future work; no SIMD or performance parity is inferred from it.
- ANS histogram lookup tables use `OnceLock` with initialized, contiguous
  Rust-owned arrays in place of C++ static initialization. Their formulas,
  numeric widths, and row order stay unchanged; initialization occurs on first
  use under the reference's default floating-point environment. This changes
  storage/initialization timing, not the search algorithm, and does not imply
  memory or performance parity. Keep `f64` rounding and `f32` cost-conversion
  points explicit. C++ `max_element`/`min_element` choose the first equal winner;
  preserve that with strict comparisons, rather than Rust's last-winner
  maximum iterator behavior. Internal writer traits express source Writer
  templates for actual output and bit-count-only estimation.
- Histogram clustering takes mutable input borrows because the C++ const
  interface writes a mutable entropy cache. Preserve those cache changes and
  first-winner ties. A Rust heap uses the original reversed `(cost, first,
  second, version)` ordering; maps retain canonical first-use reindexing. The
  scalar FastLog2f keeps the source coefficients as exact f32 bit patterns,
  integer range reduction, and separate polynomial multiplication/addition.
  Do not replace it with a library logarithm or fused operations. Disjoint
  slice borrows allow merging two histogram entries without cloning the source
  entry solely to satisfy the borrow checker.
- LZ77 retains the RLE, greedy ring-bucket, and optimal hash-chain algorithms,
  including their different special-distance collision winners and first-match
  ties. Owned vectors replace source pointers; initialized match records expose
  only fields assigned by the original dynamic program. The cost tables retain
  exact f32 bits. Upstream disables Clang auto-vectorization; on AArch64 the
  scalar histogram accumulation and final `6 - total_cost * inv_total`
  expression use fused rounding. The Rust mapping makes those rounding sites
  explicit. The x86_64 baseline has no FMA target; this is configuration-specific
  evidence, not cross-target bit parity.
  Differential regressions check all model float bits as well as chosen tokens.
- Huffman node/table pointers become indices into borrowed slices. Leaf nodes
  are sorted by population and descending symbol index, exactly as in C++;
  that comparator is a total order, so the standard library sort's internal
  implementation does not change ties. Temporary tables/arenas are initialized
  before borrowing, and only populated entries are observed. Keep signed
  16-bit node indices, unsigned count wrapping, and the original depth-limiting
  loop. Invalid internal preconditions panic rather than allowing invalid
  memory access; they do not add checks to accepted bitstream parsing. Failed
  parses preserve the source's table resize/mutation order. Symbol reads retain
  the caller's refill obligation and the reader's deferred bounds checkpoint.
- Padded storage preserves the minimum capacity, growth rule, padding, alignment,
  and rotating alignment groups. The initial backend uses Rust's allocator and
  initializes allocated bytes to zero so safe slices never expose uninitialized
  data. Additional initialization is an internal difference, not a promise of
  allocation or performance parity. Custom C allocator callbacks remain
  unported.
- Writer allotment parent pointers become indices in an owned stack. Preserve
  bit budgets, nested charging, and reclaim order. This adds Rust-managed stack
  storage; it does not change the bit-writing algorithm. The scoped callback
  receives reborrows of both the writer and optional accounting context, so
  nested writes can charge the same context without aliased mutable references.
- Complete entropy encoding retains optional output versus size-only writers,
  transposition, scalar token-cost counting, config candidate order, nested
  accounting, and prior histogram/context prefixes. The scalar MaxVectorSize
  value is four bytes; upstream adds that value to a count of u32 elements, and
  the Rust storage keeps the same padding. A newly encoded histogram writer is
  locally owned during table construction and appended even on error, retaining
  the source's observable partial state without cloning the encoding data.
  Streaming finalization accepts mutable codes because bundle visitors can
  mutate fields, following the existing bundle mapping above. The source's
  release-state fuzzer flag remains false; its debug-only global setter is not
  yet exposed. Ordinary Rust debug/release builds both match that reference.

- Color matrices use typed float/double arrays and a 16-byte aligned temporary,
  preserving source mixed-precision accumulation and inversion order. The
  translated callers have disjoint input/output storage; native borrows express
  that boundary. Ordinary Clang AArch64 expression contraction is explicit,
  including double results later narrowed through `copysignf`. C++ min/max
  operand order and unordered comparisons are retained instead of using Rust's
  different NaN rules. Opsin constants and the A2B cube keep constexpr float
  evaluation. ICC tag output remains byte-oriented, with the original profile
  date, aliasing of curve tags, padding, and unsigned MD5 wrapping. Rust's
  standard formatter supplies the source's default six-significant-digit `%g`
  output, with explicit fixed/exponent selection and C spellings; no C formatter
  is linked. Darwin omits a NaN sign, covered by an exact negative-NaN regression.
  These are reference-configuration results, not cross-platform or SIMD parity.

- Metadata names use byte vectors, because C++ strings accept arbitrary bytes
  even where comments call them UTF-8. Names and extra-channel vectors resize
  only on the source's reading branches; default visitation of a reused object
  must not clear them. Preserve unsigned field offsets before a failed visit,
  hidden color-storage synchronization, and mutations made before setter errors.
  The non-callback FromExternal color API deliberately ignores profile-generation
  failure, as the source does; field parsing propagates that failure instead.
  Static color encodings use OnceLock for the source's lazy static arrays.
  Their first-use timing is not performance evidence. Differential inputs to
  external C enums stay inside C++'s defined enum representation range; invalid
  decoded internal values still use the existing raw wrappers and validation.

Retain defined unsigned truncation and wrapping explicitly. Invalid programmer
preconditions must not cause undefined behavior in Rust. Record any discovered
source ambiguity before changing its interpretation. Do not add blanket unsafe
code or blanket lint suppression to imitate C++ syntax.

Frame and animation metadata pointers use optional shared borrows, preserving
the source's nullable cases and branch-local dereferences. Constructors assign
safe initial storage before running the original field visitor. Inactive pass
array entries have no defined initial C++ values; the oracle explicitly seeds
those entries as caller input before comparing state. Serialized fields and
the last-pass shift retain the source's mutation order. Chroma subsampling
keeps cached maxima when Set fails partway through: byte operands promote to
signed int before the difference converts to size_t. Unsigned rectangle
coordinates and group dimensions use pointer-width wrapping and the original
clamping, units, and padding; signed RectT and image-row views remain absent.
Filter validation promotes stored floats to double for the source's unsuffixed
`1e-8` threshold. Rounding that threshold to f32 incorrectly accepts the nearest
float below it; an exact differential rejection regression covers this case.
Bundle::AllDefault retains the release reference's accumulated predicate after
a failed visit; the source's JXL_DEBUG_ABORT is disabled in that configuration.
The enclosing CanEncode visit propagates its own validation error. Rust debug
and release both follow this release-state contract, with source crash-on-error
instrumentation still outside the translated boundary. The sigma regression
also checks this error path rather than converting it to a Rust panic.

Permutation coding retains the original Fenwick workspaces and their partial
mutations. Typed u16/u32 slices replace the source's permutation templates;
optional output borrows preserve validation without storing an order. A C++
integer loop can be empty when its starting index exceeds the end: retain that
with iterator take/skip rather than an invalid Rust subslice. Permutation token
counts still use unsigned subtraction and narrowing in that case. TOC vectors
clear, resize, and swap at the source's original checkpoints, including stale
permutation output when the flag is absent and unchanged offsets after failed
table reading. Rust slices correspond to non-null empty source spans; test
fixtures must not accidentally select a null reader's separate close behavior.

Plane/Image3 own the existing aligned allocation through an optional owner.
Sealed primitive numeric sample types guarantee valid byte representations,
no destructors, and alignment at most eight; locally proved typed slices borrow
initialized in-bounds storage. Mutable row/byte views require an exclusive
owner borrow. Physical row slices include padding, while copy/fill operations
use the valid width. Image3 preserves the source's first-plane stride and
per-plane failure order. Native moves transfer ownership; explicit image
cloning copies samples and transform vectors without implicit plane cloning.
Source overloads receive explicit Rust names where necessary.

Row sizing keeps the scalar source's four-byte MaxVectorSize, including rejection
of positive widths with eight-byte samples. Zero-sized planes have no allocation
context; channel reallocation inherits that null context and retains the source
failure rather than silently using a fresh allocator. Failed channel resize
keeps newly assigned logical dimensions and the old plane. C allocator callbacks
remain unported. Initialized Rust storage for max_delta_error is an internal
difference from the source's indeterminate nonserialized member. Oracle callers
seed that member and observed pixel storage explicitly. No Rust sanitizer or
memory/performance parity is inferred from C++ instrumentation or these borrows.

Scalar RCT row pointers use disjoint mutable row borrows plus index maps for
input/output permutations. A private row-access trait expresses the source's
in-place and separate-buffer calls without aliased shared/mutable references:
load all three samples before storing any output. Whole-channel inverse moves
use swaps over the same three owners, preserving component tags without plane
copies or replacement allocations. Custom thread runners and arbitrary C
pointer alias contracts remain unported. The null-pool row order and the source
uint32 task-end narrowing stay explicit. Forward scalar i32 expressions retain
the source's nonoverflowing input requirement; inverse Highway Add/Sub use
explicit wrapping. Keep the in-place forward no-op's false Status distinct from
the separate-buffer and inverse no-ops' successful Status.

Prediction row pointers become a borrowed sample slice, a position, and a signed
row stride. The slice includes owned physical padding; relative reads retain
the source boundary/interior branches and cannot outlive storage. Weighted state
borrows its immutable header and owns the original two-row buffers and per-state
reciprocal table. Keep unsigned error sums, signed fixed-point operations,
narrowing, strict magnitude ties, and division-versus-shift rounding explicit.
Weighted header parameter reads synchronize their local u32 back to i32 only
after a successful visit, preserving partial failures.

FlatDecisionNode uses raw integer slots for the original unions, with typed
accessors and native-endian packing of the two i16 property indices. This avoids
inactive-union references and invalid enum discriminants while keeping the
24-byte layout and original twice-unrolled traversal. Rust initializes storage
that C++ leaves indeterminate, including PredictionResult::multiplier without a
tree; this gives that member no added non-tree meaning. The oracle must omit
such undefined observations rather than turning extra initialization into a
parity claim. Learning predictions likewise initialize inactive entries without
giving them meaning in source observations.

Tree parsing and tokenization retain breadth-first node/leaf indices, the source
integer narrowing points, iterative property-range validation, and error-time
mutation order. Rust VecDeque replaces the source queues; work stacks own their
records and retain the same traversal order. TreeLut keeps its fixed context
array; optional offset/multiplier arrays use vectors of exactly zero or 16,384
entries because stable Rust cannot express those conditional generic array
lengths. Rust initializes storage that the source leaves indeterminate. The
oracle seeds LUT storage before observing every entry and reads only the active
union members of flattened nodes. Header transforms resize only on reading,
including when an existing bundle receives defaults.

Residual encoding writes into pre-resized caller-owned token storage, using an
index for the source's local Token pointer. Commit that index only at the source
success checkpoint; overflow leaves already written tokens and the unchanged
width output visible. Preserve builtin signed-subtraction overflow through
overflowing_sub, later i64-to-i32 narrowing, and the separate fast/slow loops.
The source's kWantDebug=true predictor-image allocation and scratch writes remain
present even though its image-output call is commented out. Decoder prediction
temporarily borrows initialized plane samples and ends that borrow before each
pixel write; no aliased mutable/shared channel pointers or whole-plane copies
are needed. Its first-row relative-pointer cases map to the same prior samples.
Shared Huffman/RLE run/value state outlives individual channel calls, and its
original global-tree predicate remains part of the fast-path selection.

Palette/Squeeze output channels own their allocations and replace the source
channel at the original assignment point. Palette index planes use the source
swap; short shared palette/prediction borrows end before each output write.
Keep source task narrowing and eight-row/64-column span order, signed tendency
division, i32 forward arithmetic preconditions, and inverse narrowing explicit.
Clamp1 retains its ordered ternary checks even for reversed bounds: an empty
single-channel palette distinguishes negative from nonnegative input indices.
Do not substitute Rust clamp or unordered min/max composition for that case.
Metadata edits retain changes completed before failure, including meta counts
and previously processed channels. New source pixel storage is indeterminate;
metadata-only oracle observations omit it. Reverse stack execution copies one
Transform as the source does, pops it after success, and leaves a failed entry
and the error flag visible. Custom runners remain a separate boundary.

ModularDecode temporarily moves its transform-vector owner out while applying
each mutable entry to the image, then restores it on success or failure.
MetaApply reads channel/size/meta state and never reads that vector, so this
avoids aliasing and additional transform copies while preserving partial entry
mutations. A Result scope performs the original armed guard's zero-filling on
every status-return path and disarms it before final ANS validation. Allowed
truncation retains its separate early-header clearing and later nonfatal undo
rules. Optional global tree/code/map inputs remain independent borrowed options;
validate their presence at the original checkpoint. Do not compare indeterminate
source pixels after allowed truncated transforms. Caller-supplied dimensions and
bit depth remain outside the standalone modular stream's serialized boundary.

TreeSamples owns the source's residual/property/count vectors and both hash
slots. Preserve resize-retained hash entries, u16 saturation/removal, strict
split ties, the source's sample partition order, and mutation of pre-quantized
pixel/difference arrays. Static properties retain their positional contract;
do not reorder or repair a caller's configuration. Learning consumes its sample
owner, while image batches and predictor headers remain borrowed. Inactive
CostInfo predictors and pre-setter fields receive safe Rust initialization with
no added source meaning. Preserve integer narrowing/wrapping and the original
unsigned-long histogram accumulation width. The one-lane Highway estimator
keeps separate float operations; ordinary Clang expressions retain the existing
target-specific contraction mapping. In particular, the unsuffixed split
penalties and required-cost constants evaluate in double before narrowing.
RNG state and geometric sampling retain logf rounding and source distribution
preconditions. Generic compression clones options at the source checkpoint,
writes its header before learning errors, and reborrows accounting for nested
writers. Caller options and transform ownership remain unchanged.

Palette selection uses ordered Rust maps/sets for the source's lexicographic
keys. A separate owned image retains lossy reconstruction samples while input
indices are written only in the final iteration. Short row borrows load all
components before writing an index. Preserve prepass predictor changes,
palette-count mutation checkpoints, implicit-color promotions, strict distance
ties, row-error swaps, and original float/double rounding. Do not assign meaning
to indeterminate source delta entries beyond its RGB lossy configuration.

For palette comparator ties, Rust's general-purpose sort does not specify the
reference C++ library's concrete order. Preserve it with a mechanical translation
of the reference libc++ non-arithmetic specialization, including insertion,
partition, pivot, and heap-fallback behavior. Its versioned/hash-checked SDK
snapshots and LLVM notices form an explicit dependency boundary. This favors
reference bitstream reproducibility over using Rust's sort. Moved-from holes
hold Default values for the source's vector/pair instantiations; those holes
are never compared. This avoids raw pointers and extra clones for element
moves. Other C++ library versions/specializations need their own evidence.

CompressParams copies owning vectors and non-owning descriptors at the source's
checkpoints. Custom spline/progressive inputs use borrowed slices/references;
they must outlive the copied parameters. CMS/debug callback signatures and raw
data pointers are stored without invocation or ownership transfer. Their C
profile/color/bit-depth layouts use explicit representation and are checked
against original size, alignment, and member offsets. This establishes data
carriers, not callback execution or the encoder C API. Initialized Rust CMS and
opaque storage has no source meaning before assignment; do not observe the
source's indeterminate members before SetCms/caller assignment. Future callback
invocation needs its own lifetime, buffer, thread, and cleanup proof.

Progressive configuration retains aggregate initialization and leaves reused
Passes entries untouched except at the source's assignments. Preserve narrowing
before zero-count rejection and the last-pass shift reset. Stream IDs retain
size_t wrapping; fixed-underlying raw kinds preserve the source switch's zero
result for unknown values. Header preparation retains byte names, copied blend
records, field narrowing, and changes completed before each validation failure.
Metadata and parsed JPEG inputs are borrowed; JPEG storage does not implement
the unported field/parser/reconstruction APIs. Source-indeterminate JPEG/spline
storage receives safe Rust initialization without extra observable meaning.

Sample conversion uses disjoint input/output slices with explicit row lengths.
Assert the source's valid-buffer and defined numeric/shift preconditions rather
than silently treating Rust's saturating float-to-int casts as source behavior.
Preserve the encoder's single/double precision threshold and original target
contraction sites. Decoder Highway additions wrap before conversion; its
explicit multiplication remains separate. Bit casts replace memcpy for custom
floats and binary32, including signed zeros and NaN payloads. Representability
errors retain prior writes and leave the remaining output storage untouched.
Original format checks that fail without accessing rows precede buffer-length
assertions; an error-only call does not require storage for an unused row.

Frame transform attempts clone the source transform, append it only at the
source success checkpoint, and pop a rejected attempt only after successful
inverse application. A failed inverse keeps its entry and returns the source's
false value, not a new status error. Forced metadata edits do not establish
defined pixel contents. Tree merging retains append offsets even in leaf child
fields; later tokenization performs the source's canonicalization. Scalar
gradient-cost residuals keep their defined signed-subtraction precondition;
weighted cost instead records overflow and rejects that mode with FLT_MAX.
Preserve per-context float-to-size_t truncation, the accumulated fraction, and
the final size_t-to-float conversion rather than summing costs in float.

ModularFrameEncoder owns its stream images/options, trees, tokens, headers,
entropy data, mappings, and copied parameters. Native image allocations retain
the existing crate allocator boundary; custom C allocators remain separate.
Preparing a group splits the stream vector into disjoint global/group borrows.
Keep channel-map insertion before allocation/copy and retain completed changes
on failure. RCT search owns scratch channels and swaps planes at the original
checkpoints; do not add atomic rollback to partially completed source work.

Every upstream call to the private PrepareStreamParams passes the encoder's
own cparams member. The Rust method borrows that field directly, removing a
same-owner argument that would otherwise require aliasing or an extra parameter
copy. Other fields remain independently borrowed. ClearStreamData swaps in an
empty owner and releases the previous image at the source scope exit; it does
not clear token, mapping, or width state. Stream options and vector resizing
retain old entries except at the source's assignments. The splitting threshold
uses size_t arithmetic because decoding_speed_tier is unsigned, including its
wrapping before float conversion. Multiplier sorting retains the reference
algorithm; safe Default values fill moved-from holes with no added meaning.

The first image configuration owns private metadata, and its frame owner borrows
that metadata while owning the header, progressive splitter, modular encoder,
and initialized quantization matrices. Its parameters contain no borrowed custom
inputs and can therefore use a static parameter lifetime. Expose read-only observations of these owners;
do not expose mutations that could select unported source branches. This is a
configuration-specific internal path, not a substitute general encoder API.

For the fixed lossless Thunder configuration, ComputeEncodingData's Gaborish,
patch search, custom splines, XYB, extra-channel conversion, channel moves,
responsive squeeze, and lossy quantization branches are unreachable through
original parameter/metadata values. Translate the reached integer conversion,
default global palette/RCT policy, group scheduling/preparation, plane release,
and validation in source order. The Image3F input can be borrowed immutably
because the only source writes to it are in the excluded effect branches.
Retain the source's double conversion factor and physical-plane versus logical
channel dimensions. General pixel-preparation entry points remain absent.

Block padding keeps its original allocation and the source Image3 stride from
plane zero. Byte copies retain float payloads and leave physical row padding
untouched. A failed ShrinkTo retains the planes already resized. Preserve the
source's unsigned DivCeil/RoundUpTo arithmetic, including defined wrapping.
An empty fill range does not construct a checked Rust row view.

Downsampling preserves nested float accumulation order and the actual sample
count at partial edges. Safe row selection replaces strided pointer arithmetic.
Allocate output through the input image's existing allocator boundary: a newly
created empty image has no allocator owner, while an allocated image shrunk to
zero still has one. These equal logical dimensions can therefore yield different
allocation statuses. Propagate the existing native-owner/null distinction to
Image3 creation; custom C allocator callbacks remain unported.

DequantMatrices owns the source's default encoding vector, strategy-to-table
offsets, empty lazy table storage, and three DC/inverse-DC values. Its constructor
retains the original block-size schedule and all initialized state. The source's
indeterminate coefficient pointers have no Rust meaning before computation;
matrix computation/accessors and coefficient libraries remain unported.
Preserve each coefficient
assignment before its validation and update the inverse only after validation.
The serialized all-default flag keeps reused values rather than restoring the
initial constants. Custom-DC round trips close their private Rust reader even
on failure, retaining the primary decode status and all earlier matrix changes;
the extra close has no externally observed reader state.
Setter inputs borrow a disjoint three-value array, matching the pinned callers'
local arrays or separate parameter storage. Aliasing matrix members through the
source's raw input pointer is not represented by this safe internal boundary.

QuantEncoding incorporates the initialization from its C++ constexpr base into
one owning Rust record. Typed payload variants replace the active union member;
do not observe inactive source bytes or give them extra meaning. Mode storage
retains the reference compiler's four-byte unsigned underlying type. Non-RAW
constructors preserve constexpr initialization. RAW borrows and copies its table
because the original named rvalue-reference argument is copied, leaving the
caller's vector unchanged. Clone duplicates its optional table owner; clone_from
releases an old RAW table before replacement. Explicit move_from transfers the
table and leaves a null owner with the original mode/metadata in the source,
while ordinary Rust moves invalidate the old binding. A moved-from null RAW
owner remains distinct from an allocated empty table. Non-owning raw-pointer
base copies are not exposed. SetEncodings copies a disjoint vector owner and
invalidates the computed mask at the source checkpoint.

The fixed profile's EncodeGroups uses an owning writer vector for the source's
unique writer owners. Preserve single-group merging, the empty global-AC slot,
DC/AC index order, and the original byte-padding/allotment layer. Null-pool task
bounds narrow to u32 and use one temporary AuxOut owner, or a null owner when
statistics are disabled. Reify the resize lambda's captures as parameters for
sequential disjoint borrows. Assimilate the worker only at the original final
resize; errors before that point drop its unassimilated statistics while keeping
earlier output mutations. Pool task failures retain the source's generic status.
The output vector's nonempty guard precedes all writes. Frame owners release the
modular encoder before its header and matrices, following reverse creation order.

The fixed profile's finalization boundary returns the local BitWriter before
TakeBytes/AppendData. Its original special-frame vector is empty and the fixed
centerfirst=false setting exits PermuteGroups at its first guard. Header
visitation mutates the real header before group-alignment validation; an error
drops the local writer while retaining header/statistics changes. Successful
TOC output precedes the byte-aligned group append. No generic permutation or
output-processor API is inferred from this boundary. The complete native
one-shot method now takes that writer's padded byte owner at AppendData's input
boundary. BitWriter::TakeBytes shrinks the existing allocation and moves it
without copying. The fixed image entry point writes the original global
metadata/padding block and appends the complete one-shot frame. Its raw byte
return replaces only native output plumbing; C output processors, callbacks,
containers, and the general image API remain unported.

AuxOut includes all source block counters and the signed iteration counter.
Unsigned sums wrap; signed overflow remains a source precondition, checked in
Rust. Layer and object assimilation preserve field order and double addition,
including signed zero and NaN bits in the reference configuration. Diagnostic
printing remains separate from this storage/accounting boundary.

External plane conversion borrows the actual input byte storage separately
from the source size argument: size=0 skips only the original size check and
does not imply an empty allocation. Slice bounds enforce the source pointer
validity precondition without unsafe loads. Keep row/channel offset arithmetic,
null-pool u32 task narrowing, float scale construction, and ordered callbacks.
The source scale shift must be below 64 even for floating input; undefined larger
shifts remain a checked programmer precondition. SafeMul/SafeAdd/SafeRoundUpTo
local checks map to checked usize arithmetic, preserving validation order and
the final source wrapping product. Pixel-format data type and endianness use raw
u32 wrappers, so unnamed enum values can follow the original guards without
invalid Rust enum discriminants. This extends the earlier endianness descriptor
to raw storage; the three named values keep their numeric representations.

The buffer-backed frame adapter borrows color input and owns copied input in
the source channel field order. Extra channels copy only after a successful
bind; CopyBuffers copies the color channel. Preserve the old copy's precedence
over a subsequently rebound pointer, including null-pointer rebindings. Empty
copies fall back to the borrowed pointer, which CopyBuffer has cleared. Input
size remains separate from the physical slice extent. SetFormatAndDimensions
retains format/dimension changes before failed size arithmetic; SafeRoundUpTo
assigns its wrapped stride even when reporting overflow. Initial Rust numeric
storage is safe, but source-indeterminate fields are observed only after their
assignment. The callback/JPEG setters remain absent, so these entry points can
select only the original buffer path. Native shared views replace its callback
table and no-op release; they cannot outlive the adapter or its input borrow.

The fixed profile's CopyColorChannels retains the default FrameInfo, including
FROM_PIXEL_FORMAT and ib_needs_color_transform=true. The original color-only
component entry point keeps a null alpha destination; the complete fixed-profile
entry point also binds its optional alpha plane by owner-relative index.
It sets the interleaved-alpha flag before requesting data and validates channel
count afterward. A disjoint borrow of the three owned planes supports gray
replication in source order without cloning storage. Failure retains completed
plane writes and the alpha flag; later planes remain untouched. This boundary
remains independently testable at the color-copy boundary. The fixed image
entry point calls it through the original outer computation order. Allocate all
extra planes before color import, copy interleaved alpha only after all color
planes, then run CopyExtraChannels and skip the first interleaved alpha request
at the original checkpoint. The buffer-only adapter retains no-op release
semantics through scoped borrows. Separate component callers may supply an
extra-channel buffer; missing buffers retain the original failure order.

Modular preparation includes each alpha plane after color in original channel
order, retaining per-channel dimensions/shifts, sample-depth conversion, and
max-bitdepth updates before the existing palette/compaction policy. The simple
render pipeline owns three color channels plus the admitted alpha channel. Its
existing modular conversion and WriteToImageBundleStage retain the alpha plane
without changing the RGB stages. Complete integer export requests gray/alpha
or RGBA when metadata has alpha. Associated alpha remains outside this profile,
so its source unpremultiplication condition is false. The original lossless
keep-invisible default prevents SimplifyInvisible; the port introduces no
replacement pixel values for transparent areas.

Quantizer stores the source scalar state and reifies its enclosing matrix
pointer as an explicit same-owner borrow for construction, recomputation, and
DC getters. This avoids a Rust self-reference when the containing frame moves.
Callers must pass the original matrix owner. Mutating that owner changes fresh
DC getters immediately while the stored multipliers retain their prior values
until recomputation. The fourth multiplier lanes have no defined source initial
values; safe Rust storage is initialized but the oracle omits those lanes until
ClearDCMul assigns them. Recompute changes only the first three lanes, retaining
a previously cleared fourth lane. Keep the original double-to-float assignments
and subsequent float arithmetic. The recomputation call stays out of line to
preserve arithmetic on signaling NaNs; the reference configuration in ADR 0002
pins the same boundary for both ordinary and instrumented C++ builds.

AcStrategyImage replaces its cached Row(0) pointer with an owner-relative offset
and retains its cached stride. Both survive native owner moves; borrowed layer
observations cannot outlive the allocation. Raw AcStrategyType uses u32 storage.
CountBlocks first truncates the type to u8, then promotes to int for the shift:
unnamed type 127 counts the 255 sentinel, 256 aliases DCT, and 128/255 compare
against values above 255. Do not wrap the shifted byte before comparing it.

The fixed frame owns the complete PassesEncoderState/PassesSharedState storage
and constructors, including four allocated reference ImageBundles and the
default BlockCtxMap. ImageFeatures owns the original patch/spline/noise storage.
The fixed-profile ComputeEncodingData retains each allocation/assignment
checkpoint, the coefficient-order size, and the original default XYB=true map
construction even for this RGB configuration. Its parameter copy occurs only
after successful pixel copying. Source image samples not initialized by the
algorithm remain omitted from observations.
ColorCorrelation default construction leaves all DC factors zero; only explicit
recomputation, setters, successful DC parsing, or map creation assign them. Its
ratio expressions retain the reference's scalar contraction.

Shared DC and patch-reference pointers become owner-relative member selectors.
The shared owner resolves its DC selector to dc_storage; the reached SetShared
records the containing reference_frames array. Matrix calls retain explicit
same-owner borrows. These bindings survive native moves without self-references.
ImageBundle borrows optional metadata and owns color, profile, extra planes,
names, and optional JPEG data. Source-indeterminate metadata, dimensions, LUTs,
and patch fields receive safe initial Rust storage without extra source meaning;
observations begin only after the original assignment. Native constructors use
the existing valid allocator context; custom C callbacks remain separate.

ACImage's two admitted template instantiations become a closed i16/i32 image
enum. Typed row-view variants replace its pointer unions and virtual dispatch;
all original allocation, stride, row, empty, and zero-fill operations are
preserved. Source unique owners become optional boxed values where null is
allowed. Splines keeps its borrowed data view and owning storage separately;
the translated constructor/Clear establishes empty views and releases both
aligned cache owners in source order. Data import/rendering remain absent.

ModularFrameDecoder owns its full modular image, deferred transform vector,
global prediction tree, ANS tables, context map, and group header. Init copies
only frame dimensions, preserving the original reused-state behavior. Global
decoding borrows the caller's header and reader for each call. A local image is
assigned to the full-image owner only after the original fatal/truncation
checks; earlier failures retain the old image and already mutated tree/table/
header state. Nonfatal truncation can still transfer that local image. Preserve
the original transform-vector copy before clearing it, and do not reset the
deferred vector or use_full_image flag at extra checkpoints. MaybeDropFullImage
releases channel planes while retaining their logical metadata. Native moves
transfer all owners; no borrowed entropy reader is stored inside the decoder.
Initial source-indeterminate flags/dimensions receive safe native storage. The
oracle explicitly seeds those flags before partial-state comparisons, and only
observes pixels that the original global decoder has initialized.

Group decoding and modular-to-pipeline conversion borrow the original decoder
state's actual shared owner and pipeline field separately. Conversion reads
only shared.matrices; finalization additionally reads shared.frame_dim and
mutates the pipeline. Optional shared-state presence retains DecodeGroup's
nullable-state checkpoint. These are field dependencies of the translated
callee, not a replacement PassesDecoderState constructor. The complete decoder
owner and initialization are translated separately; these callees retain their
field borrows. Null-pool task loops retain
uint32 bounds and the source wrapper's generic-error mapping.

Modular Image::move_from copies scalar metadata and transfers channel/transform
vectors, matching the source move assignment. It leaves the source bit depth,
dimensions, meta-channel count, and error flag intact. FinalizeDecoding reads
that retained bit depth while converting the moved image: resetting the source
to a default Image would incorrectly normalize 16-bit pixels as 8-bit pixels.
The RGB-from-gray kernel receives three disjoint mutable pipeline plane borrows
from one ordered iterator, preserving its single computation per sample without
cloning pixel storage or duplicating the kernel.

The simple render builder represents the original Builder followed by
UseSimpleImplementation; the default low-memory branch remains absent. Stage
trait defaults translate the original base bodies, including its actual empty
hooks. A mutable stage borrow expresses its SetInputSizes/PrepareForThreads and
output writes. Preserve settings reads across callbacks: border_y is captured
after SetInputSizes, and output-row shifts are read at their source checkpoints.
Do not cache an earlier settings value across that mutable callback.

Pipeline row pointers become descriptors of storage, channel, row, and column.
RowBuffers resolves them under exclusive borrows of the existing input/output
planes; it does not clone pixels or retain raw aliases. Vertical mirroring
copies byte ranges within the same owner, retaining float payloads and avoiding
scratch allocations. The horizontal offset retains the original architecture
guard (ARM 24, other supported targets 32). Allocation padding uses hwy/base.h's
kMaxVectorSize (x86 64, other baseline targets 16), independently of the scalar
MaxVectorSize() function. RVV intrinsic builds remain unported.

The scalar upsampling stage keeps three vectors of aligned temporary owners,
with the original resize/allocation order and byte counts. Existing u32 views
store/load float words without adding pointer casts or changing workspace
layout. Kernel entries not initialized by the source constructor receive safe
zero storage and remain unused. Preserve Highway's scalar min/max separately:
they ignore one NaN and choose the second operand on equality. Its scalar
MulAdd in this stage retains separate multiplication/addition in the original
compiled object, including on AArch64; it must not inherit another algorithm's
explicit fused rounding. A mirrored 3x2 regression checks the resulting words.

Gaborish uses the same row descriptors and stores its nine normalized weights
in source channel order. Its constructor keeps the ordinary reference Clang
normalization contraction. On x86, a local non-inlined binary32 multiply keeps
the reciprocal as its first operand when normalizing weights, matching the
plain source object's destination order and the instrumented object's third
weight product. The other targets retain the source compound assignments.
The instrumented x86 constructor commutes its first two products; native
ordinary/instrumented captures of competing signaling/quiet filter pairs agree
at stage output. Retain complete native parity gates for both references.
This mapping does not use Highway's widened
multiply for the constructor's ordinary float arithmetic.
The dedicated pristine scalar observation unit in ADR 0002 retains Highway's
literal Add/Mul/MulAdd boundaries. Its original operator* promotes both binary32
operands to double, multiplies, and narrows to binary32 before the separate add.
Those promotions quiet signaling NaNs before the product selects its payload.
Preserve them even when a particular optimized object folds the finite-equivalent
sequence to a binary32 multiply. Local non-inlined add, promotion, and
double-product/narrowing functions retain the original operand order and
checkpoints in Rust debug/release, with no global numeric helper change.
The kernel keeps the literal nested product-first MulAdd order rather than
borrowing fused ordinary-expression helpers or an ARM-only optimized register
schedule. Exact competing-payload row/normalization corpora and independently
captured native exceptional-value words guard this mapping. QEMU's x86
quiet-NaN payload selection is insensitive to operand order and cannot prove
native exceptional-value equivalence. Native x86 captures from both complete
original builds agree with the original ARM nine-word table. The prior finite
3x2 observation remains unchanged. Reinspect these boundaries after compiler,
source, target, linker, or instrumentation changes; other targets need their
own oracle runs.

OutputEncodingInfo initializes source-indeterminate scalar/array storage safely;
the oracle value-initializes those aggregate members explicitly. SetFromMetadata
and MaybeSetColorEncoding perform SetColorEncoding's first profile copy at the
original checkpoint, then borrow that stored copy for the remaining body. The
source argument and copied field stay equal and unchanged during those reads.
This avoids an extra profile clone when SetFromMetadata passes its own original
profile field. Subsequent writes and partial errors retain source order.
Non-XYB updates leave the old opsin matrix/default flag untouched. The
all_default_opsin intensity comparison retains the unsuffixed double subtraction
and the float threshold promoted to double; gamma and matrix coordinates narrow
at the original float assignments/call boundaries.
Setting the copied linear profile's transfer function retains its ICC bytes;
regeneration belongs to the later original caller, not to this setter.

The scalar XYB helper returns three float values instead of assigning restrict
output pointers; the stage resolves its three input rows through existing
owner-relative descriptors and stores each result in source channel order.
It copies exactly OpsinParams and the output-is-XYB predicate at construction,
independently of later caller mutations. Only the leading word of each original
four-lane inverse-matrix entry participates in the scalar helper. Local Add/Mul
call boundaries retain the source scalar operand order when native release
optimization would otherwise commute NaN operands. MulAdd remains exactly
Add(Mul(a,b),c), with separate rounding; global Highway helpers are unchanged.
Each local Mul additionally preserves scalar-inl.h's float-to-double promotions,
binary64 product, and narrowing to the float lane. Out-of-line widening and
binary64 multiplication keep native optimization from collapsing those source
checkpoints. Both XybToRgb and the scaled-output stage use this same mapping.
The helper remains out of line, and its source dataflow matches the isolated
original-source compiler configuration in ADR 0002. Captured-word regressions
cover competing NaN payloads, overflow/cancellation, and finite pixels. Review
this mapping with compiler/target changes; no performance parity is claimed.
The normal stage's scalar tail has zero length, so its MSan annotations access
no storage. Scaled XYB retains the original B-minus-Y expression and constants.
Extra channels are ignored, and failed xextra guards precede every row access.

The derived one-lane tone/HLG classes compose their actual base constructors;
their Highway bodies keep separate multiply/add operations and the original
right-nested luminance order. They do not reuse the base classes' different
pixel equations or the double-precision PQ base transfer function. TF_PQ uses
the source's exact float coefficient words, degree-four evaluation, sign
restoration, eager branch evaluation, and x86 atomic-fence sites. Scalar
FastPow2f retains Highway Floor's positive-zero result for negative zero and
its sign-based saturating NaN/infinity conversion before wrapped exponent math.
ZeroIfNegative tests the sign bit, including negative NaNs.

In the scalar GamutMap blend, the AArch64 reference Clang object emits the original
channel value as the first add operand, followed by the separately computed
product. Express that emitted operand order explicitly so a quiet-NaN channel
retains its payload. This does not change finite arithmetic or fuse operations;
exact differential and captured-word regressions cover it. The original x86_64
configuration selects the green channel's quieted NaN payload in all three
channels for the native regression's input. The actual oracle's optimized
scalar GamutMap object
uses the green product as the inner ADDSS destination and the red product as
the outer destination. Its final blend uses the separately computed product as
the ADDSS destination in every channel. Local out-of-line x86 Add calls preserve
those operand choices while keeping the original right-nested association and
separate rounding. An independent tiny source wrapper can allocate different
operands and is not a substitute for the actual oracle's object. The observed
emulated x86 runtime selects the same larger quiet-NaN payload in both operand
orders; native execution and actual-object disassembly are required to verify
these choices. The selected original O3 object emits scalar MULSS products;
its Highway double promotions are optimized away at that boundary. Keep the
AArch64 path and captured words unchanged and retain the same input in the real
differential corpus. Cross-target NaN payload equality is not assumed. Review
this mapping with compiler/target changes. ToneMappingStage retains the original
unneeded return before argument guards, then RGB load/scale/map/gamut/scale/store
order.

PassesDecoderState owns its complete shared storage and coefficient/reference
owners. Its shared pointer selects either its own storage or an actual immutable
external shared borrow; methods retain each source use of shared versus
shared_storage. Init reads sigma dimensions and transform weights through the
selected shared owner, while InitForAC always updates owned coefficient fields.
Valid source shared pointers are non-null at these method boundaries. Separate
metadata and pipeline lifetimes retain borrowed output owners; the available
pipeline type currently admits the original simple implementation only.

Shared DC bindings select owned storage or a DC-frame index in the same owner.
Preserve the assignment before a missing-frame error and the earlier level-four
guard. Owner moves keep the selector attached to the new owner. Reference-frame
patch binding still occurs before field allocation, and Create keeps its source
default xyb=true even for non-XYB metadata. No additional vector clearing or
coefficient-buffer shrinking occurs during these initialization methods.

The used-AC atomic retains 32-bit sequentially consistent stores/loads, including
the original per-strategy load in InitForAC. Callback descriptors and image-output
pointers are non-owning. PixelCallback::init is unsafe because its actual foreign
function/opaque-data contract belongs to the caller; a local proof covers the
call. Presence tests only inspect run, and Init returns the callback pointer
unchanged, including null. Descriptor drops do not call destroy. The output-stage
lifecycle and custom allocators/runners remain separate work. Oracle fixtures
explicitly seed source-indeterminate state before observing initialization.

Callback ICC setters take a mutable byte owner: failed guards/parsing retain its
allocation, while success transfers it at the original assignment. Preserve the
have_fields/want_icc updates, unknown color/transfer markers, stale gamma state,
CMYK flag assignment, and partial FromExternal mutations. Source callback output
locals receive safe native storage; successful callbacks must initialize their
complete values, and failed callbacks' output is not read.

ColorSpaceTransform owns both ICC caches and the destroy obligation for its
current foreign context. Moves transfer that obligation without copying caches.
Destroy runs before ICC storage is freed, with explicit reverse cache-release
order matching C++. Zero-capacity empty caches expose null ICC pointers in the
reference configuration; emptied allocated caches retain non-null backing.
Repeated Init keeps the original context overwrite behavior instead of adding
an extra destroy. SetICC/transform calls are unsafe at the actual foreign
function boundary, with callback/data lifetimes, read-only ICC spans, complete
output initialization, non-unwinding calls, and no owner reentry required.
General C ABI reentrancy remains outside this internal Rust boundary.

CmsStage's unsafe factory establishes the backend contract through stage drop.
Safe stage methods additionally check source programmer preconditions for input
size initialization, successful context setup, and CMS thread/pixel bounds.
Those checks guard undefined source call sequences; they do not add bitstream
validation. Stack row descriptors borrow pipeline storage, and foreign CMS
buffers use bounded raw accesses to preserve the permitted identical-pointer
channel-reduction case without aliased mutable slices. Context/profile members
are released in source order. The original IsNeeded early return invokes no
foreign function, including for the fixed lossless profile with cms_set=false.

RenderPipelineInput borrows its simple pipeline and stores owner-relative
buffer indices/rectangles. Done retains its binding on success or failure, as
in C++; explicit move_from nulls the old binding and transfers its buffer vector.
Ordinary Rust moves invalidate the old name. The borrowed token prevents moving
or aliasing pipeline storage while mutable buffers are live. This establishes
sequential null-pool behavior, not concurrent pipeline access.

ImageBundle::SetFromImage takes a mutable optional owner. Guard failures retain
Some(input); the original move consumes it into None before profile assignment
and size verification. C++ moved-from planes retain non-owning raw addresses;
native ownership does not expose those aliases as live image owners. The
destination retains its moved color/profile on a later VerifySizes error.
Profiles borrow disjoint storage, matching the real write-stage call site.
Write stages borrow the destination image/bundle directly, or borrow the same
external RefCell owner used by the sequential frame decoder. Each actual write
call holds one short exclusive borrow; the decoder can assign origin between
calls after pipeline preparation, at the original allocation checkpoint. The
write stage invokes no foreign callback while holding that borrow. This adds
no Rc, allocation, pixel copy, or competing mutable reference. The pipeline
releases its owner borrow before the completed bundle moves to the caller.
Stages own the source profile copy where required. Extra planes clear/allocate only after a
successful SetFromImage. Size getters retain the pinned JPEG-enabled build's
JPEG/color/extra-plane precedence and independent X/Y fallbacks.

The fixed raw-frame entry point keeps parsed headers private after admission,
owns its real decoder state, and follows DecodeFrame's original header/TOC,
initialization, section-reader, processing, close, and finalization checkpoints.
The header copy becomes observable only after InitFrameOutput. Section readers
borrow their physical input spans; logical section IDs retain the original
permutation assignment. Error cleanup closes every created reader without
overriding an earlier body error; explicit source close checkpoints still
propagate their own status. Early source header errors leave only a debug
close diagnostic in the release reference; native cleanup also discharges its
reader's ownership obligation without changing the returned codec status.
Source-indeterminate section/pointer/cache fields receive safe initial storage.

GroupDecCache keeps the original alignment, eleven Image3I owners, DC plane,
three aligned owners, and maximum block area. Scratch pointer bindings remain
absent until their VarDCT initializer is translated. The fixed modular path
constructs the original cache vector but never calls that initializer.
PatchDictionary::Clear empties positions/references/blendings before executing
ComputePatchTree's four vector clears and its now-certain empty-position return;
its nonempty tree builder remains a separate boundary.

Review these mappings when adding the C ABI, custom allocators, nested bundle
extensions, SIMD, or concurrency. Image kernels must not assume that the current
pilot establishes their numeric or memory behavior.

The complete fixed decoder parses metadata before transferring its owner into
an admitted configuration. Failed admission retains the parsed owner and is
outside the general codec status contract. Its frame bundle and decoder state
borrow that immutable owner; no self-referential bundle is returned. The result
owns parsed metadata and integer bytes after all image borrows have ended.
SetFromMetadata still occurs once at the original pre-padding checkpoint.
The decoder state and global reader close before external conversion, and a
prior body error takes precedence over cleanup-close status.

Integer buffer conversion uses slice-backed non-null output and no callback,
with float_out=false encoded in its function names/types. Preserve the original
1..16-bit guards, checked scanline/total-size arithmetic, four temporary channel
owners, per-thread u32 cache, nullable-channel ones, scalar NearestInt, store
order, endianness, orientation mapping, and conditional alpha unpremultiplication.
Disjoint temporary-plane borrows replace original channel-pointer rebinding.
Output allocation size and row-span validity remain caller preconditions where
the source uses restrict pointers. Separate floating buffer functions select
float_out=true and admit only 16/32-bit samples; callback entry points remain
absent. Float32 output copies words without arithmetic, preserving signaling
NaN bits. Float16 temporary planes own u16 words instead of Highway float16_t;
native-endian writes precede the source's separate byte-swap pass. Byte slices
replace typed destination casts without requiring unsafe or aliased references.

The reference Clang/GCC builds select Highway F16FromF32's native-half-operator
branch. Stable Rust has no binary16 scalar, so preserve that cast's default IEEE
nearest-even mapping explicitly. Finite conversion follows Highway's existing
mantissa rounding, carry, exponent clamping, subnormal shift, and sign restoration
algorithm. Native-cast NaNs retain their sign and high payload bits and set the
quiet bit; do not substitute Highway's alternate canonical-NaN fallback branch.
The float oracle fails compilation if HWY_HAVE_SCALAR_F16_OPERATORS is absent,
rather than claiming those different compile branches are equivalent. Exact
original-kernel comparisons cover every half word, finite half midpoints and
their binary32 neighbors, both signs, tiny/subnormal values, overflow, and NaN
payloads. Review this language-required representation for compiler/target or
floating-environment changes; alternate rounding modes and fallback branches
remain separate boundaries.

JxlEndianness value 3 is representable but unnamed and exercises the
source fall-through; value 9 produces an undefined C++ enum load and has no
assigned source codec meaning.

The x86_64 reference downsampling loop keeps its accumulator as the destination
of ADDSS. A local out-of-line scalar addition retains that operand order in
Rust Debug and Release, including competing NaN payloads. The original x86_64
DC-scaling object uses the matrix as the multiplication destination for its
first two lanes and the reciprocal scale for the third; a local out-of-line
multiplication and the same per-lane argument order retain that behavior.
AArch64 keeps its existing source expressions. These are explicit mappings of
the fixed reference compiler's scalar instructions, not changes to the sums,
scales, rounding, or error checkpoints. Review them after compiler/target
changes. Captured-word native regressions and the unchanged differential
corpora cover the observed failures.

AC matrix computation retains the original single aligned owner: the first
TOTAL_TABLE_SIZE f32 slots hold the table and the next slots hold its inverse.
Two local views rely on the existing 128-byte alignment, initialized storage,
exact byte count, and all-valid f32 bit patterns. Immutable access borrows the
owner; computation splits its exclusive view into disjoint arrays. Matrix
accessors check the source strategy/channel/computed-mask preconditions and
return bounded aligned slices instead of pointers. Uncomputed slots have no
source observation contract. Library entries preserve each original V(float)
constant as an exact f32 word with its source expression beside it; OnceLock
owns the immutable static library.

DequantMatrices::Decode recreates Library<0> owners before parsing. Retain its
initialized 27-float AFV payload as overlapping backing for partial non-RAW
writes until the original successful final mode assignment. A failure keeps
the old mode, already written/scaled payload words, and distance-band fields.
Successful decoding selects the corresponding typed payload. RAW commits its
mode and optional vector owner before reading its denominator or modular image,
then retains each copied sample before a nonpositive-sample error. Decode clears
the computed mask only after every table succeeds, preserving an existing mask
and allocation on failure. DecodeQuantTable lives beside the QuantEncoding owner;
its optional frame decoder is an immutable borrow of the same original frame
dimensions/tree/code/context owners because that source routine mutates none of
them. Native allocators and the null-pool scalar path retain their established
scope; general AC encoding and VarDCT frame orchestration remain separate work.

Scalar DCT templates retain the original radix-2 recursion, normalization,
transpose order, coefficient mutation, and scratch offsets. Valid power-of-two
sizes become checked internal parameters rather than C++ template arguments.
A single exclusive scratch slice and owner-relative offsets express recursive
in-place calls without aliased Rust borrows or replacement temporary allocations.
The source's DCTFrom/DCTTo adapters become separate bounded slices and strides;
callers retain the original disjoint-owner and valid-span preconditions.
Local arrays are safely initialized, but their source-indeterminate unused
contents are never claimed as observations. Strategy geometry and natural
coefficient orders retain the original LUTs and zigzag traversal.

The native AArch64 scalar identity loop contracts the DC residual as
sum.mul_add(-1/16, dc), matching the fixed original configuration in ADR 0002.
A local out-of-line pixel addition retains the original coefficient-first
operand order. The exact contraction matters at subnormal underflow and signed
zero as well as for NaN signs. With coefficient 0 set to binary32 word 1,
residual words 1 at positions 2/4/6/16/18/20/22/32, and all other words zero,
both the default original and selected original builds give word 0 at pixel
index 14 for stride 13; a separate multiplication/subtraction would give word 1.
Other targets retain their existing ordinary-expression mapping and require
native differential evidence.

Before the new fixture's unrolling boundary was fixed, the optimized ordinary
AppleClang 21 AArch64 object emitted FMSUB, whereas its ASan/UBSan object emitted
FMADD with a negative scale. For 64 coefficients set to 0.125 except coefficient
21 set to 0x7fc12345, IDENTITY, and stride 13, the original output words at index
4 were respectively 0xffc12345 and 0x7fc12345; both gave 0x7fc12345 at index 19.
Actual complete-adapter objects establish those observations. With the loop
shape retained, both original graphs emit FMADD and give 0x7fc12345 at both
indices. The captured native AArch64 regression follows that fixed configuration
and independently checks its fused finite underflow result. All exception inputs
and exact comparisons remain enabled; the original archive and existing fixture
flags remain unchanged. Review this mapping after compiler, source, target,
or instrumentation changes; SIMD and VarDCT frame decoding remain unported.

Competing-payload tests additionally retain the selected AArch64 IDCT object's
addend-first FADD, AFV's coefficient-1-first final DC sum, and its
accumulator-first basis FADD. Local out-of-line additions keep those observed
operand choices in native debug/release builds without changing association,
normalization, or finite results. The original length-one scaled DCT folds its
constant multiply-by-one into raw loads/stores in both selected objects. Retain
that copy so a signaling coefficient is not quieted before a later competing
addition. Native captures from both complete original graphs check the distinct
DCT/AFV winning payloads, the later LLF signaling-NaN winner, and the length-one
unquieted copy. These are target/configuration mappings, not cross-target NaN
payload equivalence or a replacement of the original scalar algorithms.

Native x86_64 Linux run 41345075028, job 124963054935, separately exercised
both complete ordinary and ASan/UBSan original graphs. For an 8x8 DCT with
stride 11, coefficients 0/1 set to 0x7fc12345/0xffc98765, and every other
coefficient set to 0.125, both original graphs retain 0x7fc12345 at pixel zero.
The earlier product-first Rust addition instead selected 0xffc98765 in 32
pixels while coefficients and scratch already matched. The local IDCT addition
therefore keeps the addend first on both native targets; an x86 captured
regression retains that original word. The competing-payload corpus still
compares every complete observation exactly, collecting all mismatches before
failing. Linux diagnostics inspect the actual ordinary and instrumented DCT
objects as well as the Rust helpers; small arithmetic probes and emulation do
not establish the original objects' operand selection.

The next native Linux run, 41347513370 / job 124970408958, matches every
IDCT, identity, and LLF request. Its two complete original graphs expose the
same 24 AFV differences: strategies 14 through 17, six ordered NaN pairs at
coefficients 0/1, and the 16 AFV-quadrant pixels only. Coefficients, scratch,
padding, and the other 210 differential groups match. In the ordinary original
DctCommand object, offsets 0x31c6/0x31ca/0x31cd add c to a, copy b, then add
(a+c) into b. The instrumented object's 0x374d/0x3751/0x3754 retain the same
operand order. Reuse the existing local afv_add(b, a+c) on both native targets.

The actual ordinary basis loop at 0x324a adds the product into the accumulator;
the instrumented AFVIDCT4x4 at 0x87/0x8d multiplies the product and adds it into
the accumulator too. Preserve accumulator-first addition on x86 as on ARM.
Another 24 exact requests place competing payloads at coefficients 2/16, which
both enter that basis loop, across all four orientations. Retain every prior
request and expected word. These changes leave the selected ARM operations,
association, finite arithmetic, compiler flags, and original source unchanged.

### Packed coefficient-order decoding

The original CoeffOrderOffset macro is a const function over the unchanged
40-entry offset table. DecodeCoeffOrders receives the existing packed u32 order
storage as a mutable slice. Each accessed channel range covers the selected
strategy bucket's full coefficient count; unused buckets need no writes.
Internal optional slices represent original null outputs for encoded unused
orders, while the real entropy symbols and final state are still validated.

The source's first-representative strategy loop and masks remain explicit.
Natural-order scratch grows only when required and is filled by the existing
AcStrategy traversal. A permutation is fully read before its destination is
Lehmer-decoded and mapped through the natural order. No synthetic output or
rollback is added after an error; already completed channels remain observable.
Zero used_orders leaves the reader untouched; nonzero reserved bits still run
histogram initialization and final-state validation. The new original-reader
fixture keeps eight leading and trailing bytes of real backing while exposing
only its logical input span.

### Quantizer parameter and scale boundary

QuantizerParams uses the existing bundle visitor and the original two U32
distributions. Decode reads into a local parameter object and assigns the
quantizer only after successful parsing; reader failure/close state remains
observable. GetParams preserves signed-to-unsigned field conversion. The matrix
owner remains an explicit same-owner borrow for decode, recomputation, scale
changes, and matrix slices. No new stored pointer or runtime dependency is added.

ScaleGlobalScale retains binary32 multiplication, ties-away-from-zero lround,
ratio computation before assignment, and recomputation afterward. Its native
precondition bounds the rounded product to the target C long domain and
excludes nonfinite products. The rounded long then narrows to i32 with the
selected original low-bit integer conversion, including results outside i32.
For global_scale=2147483647 and scale=1, the binary32 product is 2147483648;
both ordinary and instrumented originals narrow it to -2147483648 and return
ratio word 3212836864. This is integer narrowing after lround, not an undefined
floating-to-i32 cast. ClampVal uses ordered comparisons corresponding to
std::max(1, std::min(value, kQuantMax)), including quiet NaNs and infinities.

The selected original optimized AArch64 ClampVal lowers its inner minimum to
FCMP/FCSEL, its outer maximum to FMAXNM, and the cast to FCVTZS. The exact request
`qz-clamp 4 2143363909 4290917462 2141266757 4288820310` gives
`C 1 1 0 0` in the ordinary original. The complete ASan/UBSan original instead
aborts at pristine quantizer.h:70:9 with `nan is outside the range of
representable values of type 'int'` on the first signaling NaN. Those inputs
therefore do not establish a defined portable ClampVal integer result and are
excluded from that new method's differential domain. This is a narrow observed
source/compiler boundary, not a claim about every NaN arithmetic operation or
every target. Constructor, decode, and defined scaling arithmetic retain their
signaling-NaN cases, and no prior expected words, numeric flags, or sanitizer
suppressions change.

### VarDCT block-context helpers and wire format

BlockCtxMap retains its existing owner and public vectors. Context lookup keeps
the original channel swap, threshold comparisons, packed order/DC index, and
caller-provided map bounds. Bucket calculations retain pointer-width arithmetic
and source unsigned narrowing. Density normalization uses the original two
64-entry tables; reached normalized count and position are in 1..63, and coverage
is the power of two described by its logarithm. Predictor neighbor slices cover
the reached elements, and each reached signed addition fits i32 as in C++.

DecodeBlockCtxMap replaces the whole owner on its default bit. Otherwise it
updates DC threshold vectors and their product, then QF thresholds, before the
64-partition guard. The map is resized before genuine context-map decoding;
errors preserve that partial state and the original num_ctxs update order.
Threshold U32 additions narrow/wrap in the same place as the source. Encoding
uses the same distributions, caller counts that fit four bits, and nested AC
layer accounting. No new pointers, unsafe code, dependency, numeric option, or
sanitizer suppression is introduced.

### AC block entropy symbols

DecodeACVarBlock retains its source argument boundary. Existing AcPtrMut typed
slice variants replace ACPtr and the ACType template parameter; a const bool
retains uses_lz77 specialization over the genuine symbol reader. Neighbor rows,
coefficient storage, context maps, and packed order storage remain caller
borrows with the original valid-index and signed-predictor preconditions.

Nonzero context calculation narrows from pointer width to i32 at the source
assignment, then promotes back for symbol lookup. Coefficient reconstruction
uses unsigned magnitude/sign/left-shift first, followed by the source ptrdiff_t
conversion and low-bit signed narrowing selected by Clang. Shift is below the
pointer width and reached ptrdiff_t additions must be representable. Defined
inputs retain identical additions, row updates, early errors, and partial
mutation. The test adapter extracts only the original template with a pinned
SHA-256 and calls actual entropy machinery in ordinary and fully instrumented
original graphs. No unsafe Rust, new dependency, numeric flag, or blanket
suppression is added.

VarDCT DC/AC metadata methods keep the source's selected-shared reads and owned
shared-storage writes distinct. Decode failures retain already written CfL,
sharpness, strategy, and QF cells. Strategy setters preserve overlap checks and
partial writes; parsed used_acs is atomically merged only at the original final
checkpoint. Image row/rectangle views borrow their owners and retain physical
padding. Signed modular-to-map conversion clamps to the original int8 range.

Adaptive DC smoothing retains the existing scalar Highway multiplication's
binary64 promotion/narrowing and separate additions. Out-of-line boundaries
preserve competing NaN operand order rather than rewriting NaN bits. Its selected
pristine ordinary/instrumented objects agree on retained exceptional requests;
all finite observations also match both default optimized source graphs.
DequantDC preserves quant bucket assignment, inverse scales, signed plane reads,
CfL arithmetic and original output writes. ComputeSigma retains source shifted
coordinates, border copies, and ordered std::min behavior for NaNs and ties.

AC group cache owners expose aligned typed scratch slices through disjoint
borrows. A borrowed group-state view separates shared fields/code/orders from
mutable coefficient storage and pipeline input. Physical section readers move
into the original per-pass scheduling order and move back before propagating
an error, retaining their close obligations and exact partially consumed state.
Neither reader state nor complete storage owners are cloned to hide aliasing.
Dequantization preserves unsigned source bit operations, scalar multiplication,
ordered CfL additions and coefficient-width narrowing. Source-derived traversal
fixtures compare initialized pipeline pixels, owners, callback order and partial
state for every strategy, both widths and draw/accumulation modes.

The first complete VarDCT output uses TF_SRGB::EncodedFromDisplay because the
pinned common.h defaults JXL_HIGH_PRECISION to 1. Retain its degree-four rational
polynomials, square-root and sign restoration; FastLinearToSRGB is a separately
translated helper and has its own exact corpus. No approximate replacement or
float tolerance is used to make complete-file results pass.


The scalar EPF stages retain the original filter copy, neighbor/SAD accumulation
order, high-precision reciprocal and scalar Highway multiplication checkpoints.
Signed extended-row coordinates convert to block indices at the original source
points. Sigma binding is either an immutable borrowed plane or the same
`Rc<RefCell<Plane<f32>>>` owner retained by the decoder. Each row call borrows that
owner immutably and releases it before return; sigma generation requires an
exclusive borrow between stage calls. No sigma snapshot or self-reference is
created, and subsequent source-owner writes remain visible to the existing stage.
The pipeline order remains EPF0 for at least three iterations, EPF1 for at least
one, and EPF2 for at least two. Scalar operator boundaries preserve the selected
original reference configuration in ADR 0002; other target specializations remain
separate work.
Spot-color stages retain a borrow of the original four-coefficient array rather
than snapshotting it. The ordinary factory borrows `[f32; 4]`, matching immutable
frame metadata in the reached decoder pipeline; the `Cell<f32>` array factory
permits safe writes to that same external pointee between calls. Alpha scale is
read once after the extra-span guard, while each color coefficient is read at
the original channel/x loop point. No metadata owner is cloned or redesigned.
The source's `size_t` channel-offset addition wraps explicitly; wrapped RGB
aliases are query-only because processing them would violate the original
`JXL_RESTRICT` row-pointer precondition. RGB arithmetic uses the existing target
scalar multiply/add mapping, with exact operand-word coverage against the actual
original stage rather than a new interpolation formula.
