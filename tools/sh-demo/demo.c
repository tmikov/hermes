
#include "hermes/VM/static_h.h"

#include <stdlib.h>


static uint32_t unit_index;
static inline SHSymbolID* get_symbols(SHUnit *);
static inline SHPropertyCacheEntry* get_prop_cache(SHUnit *);
static const SHSrcLoc s_source_locations[];
static SHNativeFuncInfo s_function_info_table[];
static SHLegacyValue _0_global(SHRuntime *shr);
static SHLegacyValue _1_createCounterWithGenerator(SHRuntime *shr);
static SHLegacyValue _2_show_tdz(SHRuntime *shr);
static SHLegacyValue _3_get_second(SHRuntime *shr);
static SHLegacyValue _4_PrototypeClass(SHRuntime *shr);
static SHLegacyValue _5_first(SHRuntime *shr);
static SHLegacyValue _6_DerivedClass(SHRuntime *shr);
static SHLegacyValue _7_second(SHRuntime *shr);
static SHLegacyValue _8_doSteps(SHRuntime *shr);
static SHLegacyValue _9_increment(SHRuntime *shr);
static SHLegacyValue _10_doSteps_inner(SHRuntime *shr);
// demo.js:1:1
static SHLegacyValue _0_global(SHRuntime *shr) {
  _SH_MODEL();
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
    SHLegacyValue t3;
    SHLegacyValue t4;
    SHLegacyValue t5;
    SHLegacyValue t6;
    SHLegacyValue t7;
    SHLegacyValue t8;
    SHLegacyValue t9;
    SHLegacyValue t10;
    SHLegacyValue t11;
    SHLegacyValue t12;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 12);
  locals.head.count =13;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();
  locals.t3 = _sh_ljs_undefined();
  locals.t4 = _sh_ljs_undefined();
  locals.t5 = _sh_ljs_undefined();
  locals.t6 = _sh_ljs_undefined();
  locals.t7 = _sh_ljs_undefined();
  locals.t8 = _sh_ljs_undefined();
  locals.t9 = _sh_ljs_undefined();
  locals.t10 = _sh_ljs_undefined();
  locals.t11 = _sh_ljs_undefined();
  locals.t12 = _sh_ljs_undefined();

  SHJmpBuf jmpBuf;
  volatile uint32_t tryState = 0;
  if (__builtin_expect(_sh_try(shr, &jmpBuf), 0) != 0) goto L_catch;

L0:
  ;
  locals.t2 = _sh_ljs_create_environment(shr, NULL, 1);
  _sh_ljs_declare_global_var(shr, get_symbols(shUnit)[7] /*createCounterWithGen...*/);
  _sh_ljs_declare_global_var(shr, get_symbols(shUnit)[8] /*show_tdz*/);
  locals.t0 = _sh_ljs_create_closure(shr, &locals.t2, _1_createCounterWithGenerator, &s_function_info_table[1], shUnit);
  locals.t1 = _sh_ljs_get_global_object(shr);
  _sh_ljs_put_by_id_loose_rjs(shr,&locals.t1, get_symbols(shUnit)[7] /*createCounterWithGen...*/, &locals.t0, get_prop_cache(shUnit) + 0);
  locals.t0 = _sh_ljs_create_closure(shr, &locals.t2, _2_show_tdz, &s_function_info_table[2], shUnit);
  _sh_ljs_put_by_id_loose_rjs(shr,&locals.t1, get_symbols(shUnit)[8] /*show_tdz*/, &locals.t0, get_prop_cache(shUnit) + 1);
  locals.t5 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 2);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t5,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 3);
  locals.t4 = _sh_ljs_undefined();
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[11] /*Closures\n=========*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t5;
  locals.t0 = _sh_ljs_call(shr, frame, 1);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t1,get_symbols(shUnit)[7] /*createCounterWithGen...*/, get_prop_cache(shUnit) + 4);
  frame[6] = _sh_ljs_undefined();
  frame[4] = _sh_ljs_undefined();
  locals.t5 = _sh_ljs_call(shr, frame, 0);
  locals.t7 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 5);
  locals.t6 = _sh_ljs_get_by_id_rjs(shr,&locals.t7,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 6);
  locals.t0 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[12] /*HermesInternal*/, get_prop_cache(shUnit) + 7);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[13] /*concat*/, get_prop_cache(shUnit) + 8);
  frame[4] = _sh_ljs_get_string(shr, get_symbols(shUnit)[14] /*Generating */);
  frame[2] = _sh_ljs_get_string(shr, get_symbols(shUnit)[15] /* increments with def...*/);
  frame[6] = _sh_ljs_undefined();
  frame[3] = _sh_ljs_double(5);
  frame[3] = _sh_ljs_call(shr, frame, 2);
  frame[6] = _sh_ljs_undefined();
  frame[5] = locals.t6;
  frame[4] = locals.t7;
  locals.t0 = _sh_ljs_call(shr, frame, 1);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t5,get_symbols(shUnit)[3] /*doSteps*/, get_prop_cache(shUnit) + 9);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t5;
  frame[3] = _sh_ljs_double(5);
  locals.t6 = _sh_ljs_call(shr, frame, 1);
  // AllocStackInst
  // AllocStackInst
  locals.t0 = locals.t6;
  locals.t3 = _sh_ljs_iterator_begin_rjs(shr, &locals.t0);
  goto L1;
L1:
  ;
  locals.t6 = locals.t0;
  locals.t8 = _sh_ljs_iterator_next_rjs(shr, &locals.t3, &locals.t6);
  locals.t6 = locals.t3;
  locals.t6 = _sh_ljs_bool(locals.t6.raw == locals.t4.raw);
  if(_sh_ljs_get_bool(locals.t6)) goto L5;
  goto L2;

L2:
  ;
  tryState = 1;
  goto L3;

L3:
  ;
  locals.t7 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 10);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t7,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 11);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t7;
  frame[3] = locals.t8;
  locals.t6 = _sh_ljs_call(shr, frame, 1);
  tryState = 0;
  goto L1;

L4:
  ;
  tryState = 0;
  locals.t0 = _sh_get_clear_thrown_value(shr);
  locals.t3 = locals.t3;
  _sh_ljs_iterator_close_rjs(shr, &locals.t3, true);
  _sh_throw(shr, locals.t0);

L5:
  ;
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 12);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 13);
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[16] /*Further increments:*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  locals.t0 = _sh_ljs_call(shr, frame, 1);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t5,get_symbols(shUnit)[3] /*doSteps*/, get_prop_cache(shUnit) + 14);
  frame[3] = _sh_ljs_double(3);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t5;
  locals.t5 = _sh_ljs_call(shr, frame, 1);
  // AllocStackInst
  // AllocStackInst
  locals.t0 = locals.t5;
  locals.t3 = _sh_ljs_iterator_begin_rjs(shr, &locals.t0);
  goto L6;
L6:
  ;
  locals.t5 = locals.t0;
  locals.t7 = _sh_ljs_iterator_next_rjs(shr, &locals.t3, &locals.t5);
  locals.t5 = locals.t3;
  locals.t5 = _sh_ljs_bool(locals.t5.raw == locals.t4.raw);
  if(_sh_ljs_get_bool(locals.t5)) goto L10;
  goto L7;

L7:
  ;
  tryState = 2;
  goto L8;

L8:
  ;
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 15);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 16);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t7;
  locals.t5 = _sh_ljs_call(shr, frame, 1);
  tryState = 0;
  goto L6;

L9:
  ;
  tryState = 0;
  locals.t0 = _sh_get_clear_thrown_value(shr);
  locals.t3 = locals.t3;
  _sh_ljs_iterator_close_rjs(shr, &locals.t3, true);
  _sh_throw(shr, locals.t0);

L10:
  ;
  locals.t5 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 17);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t5,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 18);
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[17] /*\nTDZ\n=========*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t5;
  locals.t0 = _sh_ljs_call(shr, frame, 1);
  tryState = 3;
  goto L11;

L11:
  ;
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t1,get_symbols(shUnit)[8] /*show_tdz*/, get_prop_cache(shUnit) + 19);
  frame[6] = _sh_ljs_undefined();
  frame[4] = _sh_ljs_undefined();
  locals.t0 = _sh_ljs_call(shr, frame, 0);
  tryState = 0;
  goto L13;

L12:
  ;
  tryState = 0;
  locals.t0 = _sh_get_clear_thrown_value(shr);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 20);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 21);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[18] /*stack*/, get_prop_cache(shUnit) + 22);
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[19] /*TDZ Error!*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  locals.t0 = _sh_ljs_call(shr, frame, 2);
  goto L13;
L13:
  ;
  locals.t0 = _sh_ljs_new_object(shr);
  locals.t3 = _sh_ljs_create_closure(shr, &locals.t2, _3_get_second, &s_function_info_table[3], shUnit);
  locals.t11 = _sh_ljs_get_string(shr, get_symbols(shUnit)[20] /*second*/);
  _sh_ljs_put_own_getter_setter_by_val(shr, &locals.t0, &locals.t11, &locals.t3, &locals.t4, 1);
  frame[2] = _sh_ljs_new_object_with_buffer(shr, shUnit, 0, 0);
  // ImplicitMovInst
  // ImplicitMovInst
  // ImplicitMovInst
  frame[3] = locals.t0;
  locals.t3 = _sh_ljs_call_builtin(shr, frame, 2, 39);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 23);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 24);
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[21] /*\nPrototypical Inheri...*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  locals.t3 = _sh_ljs_call(shr, frame, 1);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 25);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 26);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[0] /*first*/, get_prop_cache(shUnit) + 27);
  locals.t10 = _sh_ljs_get_string(shr, get_symbols(shUnit)[22] /*First property:*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t10;
  locals.t3 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 28);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 29);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[20] /*second*/, get_prop_cache(shUnit) + 30);
  locals.t9 = _sh_ljs_get_string(shr, get_symbols(shUnit)[23] /*Second property:*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t9;
  locals.t3 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 31);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 32);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 33);
  locals.t8 = _sh_ljs_get_string(shr, get_symbols(shUnit)[25] /*Third property:*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t8;
  locals.t3 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 34);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 35);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[20] /*second*/, get_prop_cache(shUnit) + 36);
  locals.t7 = _sh_ljs_get_string(shr, get_symbols(shUnit)[26] /*Second property agai...*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t7;
  locals.t3 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 37);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 38);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 39);
  locals.t3 = _sh_ljs_get_string(shr, get_symbols(shUnit)[27] /*Third property now:*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t3;
  locals.t0 = _sh_ljs_call(shr, frame, 2);
  // AllocStackInst
  locals.t0 = _sh_ljs_create_class(shr,&locals.t2, _4_PrototypeClass, &s_function_info_table[4], (SHUnit *)shUnit, &locals.t5, NULL);
  locals.t12 = locals.t5;
  locals.t6 = _sh_ljs_create_closure(shr, &locals.t2, _5_first, &s_function_info_table[5], shUnit);
  locals.t5 = _sh_ljs_get_string(shr, get_symbols(shUnit)[0] /*first*/);
  _sh_ljs_put_own_getter_setter_by_val(shr, &locals.t12, &locals.t5, &locals.t6, &locals.t4, 0);
  // AllocStackInst
  locals.t0 = _sh_ljs_create_class(shr,&locals.t2, _6_DerivedClass, &s_function_info_table[6], (SHUnit *)shUnit, &locals.t5, &locals.t0);
  locals.t6 = locals.t5;
  locals.t5 = _sh_ljs_create_closure(shr, &locals.t2, _7_second, &s_function_info_table[7], shUnit);
  _sh_ljs_put_own_getter_setter_by_val(shr, &locals.t6, &locals.t11, &locals.t5, &locals.t4, 0);
  _sh_ljs_store_to_env(shr, locals.t2,locals.t0, 0);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 40);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 41);
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[28] /*\nClasses\n=========*/);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  locals.t2 = _sh_ljs_call(shr, frame, 1);
  frame[6] = locals.t0;
  frame[5] = locals.t0;
  frame[4] = _sh_ljs_undefined();
  locals.t0 = _sh_ljs_call(shr, frame, 0);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 42);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 43);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[0] /*first*/, get_prop_cache(shUnit) + 44);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t10;
  locals.t2 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 45);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 46);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[20] /*second*/, get_prop_cache(shUnit) + 47);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t9;
  locals.t2 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 48);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 49);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 50);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t8;
  locals.t2 = _sh_ljs_call(shr, frame, 2);
  locals.t6 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 51);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t6,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 52);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[20] /*second*/, get_prop_cache(shUnit) + 53);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t6;
  frame[3] = locals.t7;
  locals.t2 = _sh_ljs_call(shr, frame, 2);
  locals.t2 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t1, get_symbols(shUnit)[9] /*console*/, get_prop_cache(shUnit) + 54);
  frame[5] = _sh_ljs_get_by_id_rjs(shr,&locals.t2,get_symbols(shUnit)[10] /*log*/, get_prop_cache(shUnit) + 55);
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 56);
  frame[6] = _sh_ljs_undefined();
  frame[4] = locals.t2;
  frame[3] = locals.t3;
  locals.t0 = _sh_ljs_call(shr, frame, 2);
  _sh_end_try(shr, &jmpBuf);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;

L_catch:
  if (tryState == 0) {
    _sh_end_try(shr, &jmpBuf);
    _sh_throw_current(shr);
  }
  _sh_catch_no_pop(shr, (SHLocals*)&locals, frame, 12);

  switch (tryState) {
    default:
      abort();
    case 2:
      goto L9;
    case 3:
      goto L12;
    case 1:
      goto L4;
  }
}
// demo.js:1:1
static SHLegacyValue _1_createCounterWithGenerator(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 2);
  locals.head.count =3;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  locals.t0 = _sh_ljs_get_env_from_closure(shr, frame[-7]);  locals.t1 = _sh_ljs_create_environment(shr, &locals.t0, 2);
  np0 = _sh_ljs_double(0);
  _sh_ljs_store_to_env(shr, locals.t1,np0, 0);
  locals.t2 = _sh_ljs_create_closure(shr, &locals.t1, _9_increment, &s_function_info_table[9], shUnit);
  _sh_ljs_store_to_env(shr, locals.t1,locals.t2, 1);
  locals.t0 = _sh_ljs_new_object_with_buffer(shr, shUnit, 1, 3);
  _sh_prstore_object(shr, &locals.t0, 0, &locals.t2);
  locals.t1 = _sh_ljs_create_closure(shr, &locals.t1, _8_doSteps, &s_function_info_table[8], shUnit);
  _sh_prstore_object(shr, &locals.t0, 1, &locals.t1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:31:1
static SHLegacyValue _2_show_tdz(SHRuntime *shr) {
  struct {
    SHLocals head;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 2);
  locals.head.count =0;
  SHUnit *shUnit = shr->units[unit_index];
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  np0 = _sh_ljs_undefined();
  _sh_leave(shr, &locals.head, frame);
  return np0;
}
// demo.js:52:5
static SHLegacyValue _3_get_second(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 11);
  locals.head.count =3;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  locals.t0 = _sh_ljs_coerce_this_ns(shr, frame[-8]);
  locals.t1 = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 57);
  if(_sh_ljs_to_boolean(locals.t1)) goto L2;
  goto L1;

L1:
  ;
  np0 = _sh_ljs_double(1);
  _sh_ljs_put_by_id_loose_rjs(shr,&locals.t0, get_symbols(shUnit)[24] /*third*/, &np0, get_prop_cache(shUnit) + 58);
  goto L3;
L2:
  ;
  locals.t1 = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 59);
  locals.t1 = _sh_ljs_inc_rjs(shr, &locals.t1);
  _sh_ljs_put_by_id_loose_rjs(shr,&locals.t0, get_symbols(shUnit)[24] /*third*/, &locals.t1, get_prop_cache(shUnit) + 60);
  goto L3;
L3:
  ;
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 61);
  locals.t0 = _sh_ljs_get_global_object(shr);
  locals.t0 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t0, get_symbols(shUnit)[12] /*HermesInternal*/, get_prop_cache(shUnit) + 62);
  frame[4] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[13] /*concat*/, get_prop_cache(shUnit) + 63);
  np0 = _sh_ljs_undefined();
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[29] /*Getter executed, thi...*/);
  frame[5] = _sh_ljs_undefined();
  locals.t0 = _sh_ljs_call(shr, frame, 1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:76:5
static SHLegacyValue _4_PrototypeClass(SHRuntime *shr) {
  struct {
    SHLocals head;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 2);
  locals.head.count =0;
  SHUnit *shUnit = shr->units[unit_index];
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  np0 = _sh_ljs_undefined();
  _sh_leave(shr, &locals.head, frame);
  return np0;
}
// demo.js:79:5
static SHLegacyValue _5_first(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 2);
  locals.head.count =1;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();

L0:
  ;
  locals.t0 = _sh_ljs_get_string(shr, get_symbols(shUnit)[1] /*I am in the prototyp...*/);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:85:5
static SHLegacyValue _6_DerivedClass(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 10);
  locals.head.count =3;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();

L0:
  ;
  locals.t2 = frame[-6];
  locals.t0 = _sh_ljs_get_env_from_closure(shr, frame[-7]);  locals.t0 = _sh_ljs_load_from_env(locals.t0, 0);
  locals.t0 = _sh_ljs_load_parent_no_traps(shr, locals.t0);
  locals.t1 = _sh_ljs_create_this(shr, &locals.t0, &locals.t2, get_prop_cache(shUnit) + 64);
  frame[4] = locals.t2;
  frame[3] = locals.t0;
  frame[2] = locals.t1;
  locals.t0 = _sh_ljs_call(shr, frame, 0);
  locals.t2 = _sh_ljs_empty();
  _sh_ljs_throw_if_this_initialized(shr, locals.t2);
  locals.t0 = _sh_ljs_is_object(locals.t0) ? locals.t0 : locals.t1;
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:89:5
static SHLegacyValue _7_second(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 11);
  locals.head.count =3;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  locals.t0 = frame[-8];
  locals.t1 = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 65);
  if(_sh_ljs_to_boolean(locals.t1)) goto L2;
  goto L1;

L1:
  ;
  np0 = _sh_ljs_double(1);
  _sh_ljs_put_by_id_strict_rjs(shr,&locals.t0, get_symbols(shUnit)[24] /*third*/, &np0, get_prop_cache(shUnit) + 66);
  goto L3;
L2:
  ;
  locals.t1 = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 67);
  locals.t1 = _sh_ljs_inc_rjs(shr, &locals.t1);
  _sh_ljs_put_by_id_strict_rjs(shr,&locals.t0, get_symbols(shUnit)[24] /*third*/, &locals.t1, get_prop_cache(shUnit) + 68);
  goto L3;
L3:
  ;
  frame[2] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[24] /*third*/, get_prop_cache(shUnit) + 69);
  locals.t0 = _sh_ljs_get_global_object(shr);
  locals.t0 = _sh_ljs_try_get_by_id_rjs(shr,&locals.t0, get_symbols(shUnit)[12] /*HermesInternal*/, get_prop_cache(shUnit) + 70);
  frame[4] = _sh_ljs_get_by_id_rjs(shr,&locals.t0,get_symbols(shUnit)[13] /*concat*/, get_prop_cache(shUnit) + 71);
  np0 = _sh_ljs_undefined();
  frame[3] = _sh_ljs_get_string(shr, get_symbols(shUnit)[29] /*Getter executed, thi...*/);
  frame[5] = _sh_ljs_undefined();
  locals.t0 = _sh_ljs_call(shr, frame, 1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:6:5
static SHLegacyValue _8_doSteps(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 2);
  locals.head.count =2;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  locals.t0 = _sh_ljs_get_env_from_closure(shr, frame[-7]);  locals.t0 = _sh_ljs_create_environment(shr, &locals.t0, 5);
  locals.t1 = _sh_ljs_param(frame, 1);
  _sh_ljs_store_to_env(shr, locals.t0,locals.t1, 0);
  np0 = _sh_ljs_double(0);
  _sh_ljs_store_to_env(shr, locals.t0,np0, 3);
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  locals.t0 = _sh_ljs_create_generator_object(shr, &locals.t0, _10_doSteps_inner, &s_function_info_table[10], shUnit);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:4:23
static SHLegacyValue _9_increment(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 2);
  locals.head.count =3;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();
  SHLegacyValue np0 = _sh_ljs_undefined();

L0:
  ;
  locals.t1 = _sh_ljs_get_env_from_closure(shr, frame[-7]);  locals.t2 = _sh_ljs_param(frame, 1);
  np0 = _sh_ljs_undefined();
  np0 = _sh_ljs_bool(locals.t2.raw != np0.raw);
  if(_sh_ljs_get_bool(np0)) goto L2;
  goto L1;

L1:
  ;
  locals.t2 = _sh_ljs_double(1);
  goto L2;
L2:
  ;
  // PhiInst
  locals.t0 = _sh_ljs_load_from_env(locals.t1, 0);
  locals.t0 = _sh_ljs_add_rjs(shr, &locals.t0, &locals.t2);
  _sh_ljs_store_to_env(shr, locals.t1,locals.t0, 0);
  _sh_leave(shr, &locals.head, frame);
  return locals.t0;
}
// demo.js:6:5
static SHLegacyValue _10_doSteps_inner(SHRuntime *shr) {
  struct {
    SHLocals head;
    SHLegacyValue t0;
    SHLegacyValue t1;
    SHLegacyValue t2;
    SHLegacyValue t3;
  } locals;
  _sh_check_native_stack_overflow(shr);
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 10);
  locals.head.count =4;
  SHUnit *shUnit = shr->units[unit_index];
  locals.t0 = _sh_ljs_undefined();
  locals.t1 = _sh_ljs_undefined();
  locals.t2 = _sh_ljs_undefined();
  locals.t3 = _sh_ljs_undefined();
  SHLegacyValue np0 = _sh_ljs_undefined();
  SHLegacyValue np1 = _sh_ljs_undefined();
  SHLegacyValue np2 = _sh_ljs_undefined();
  SHLegacyValue np3 = _sh_ljs_undefined();
  SHLegacyValue np4 = _sh_ljs_undefined();
  SHLegacyValue np5 = _sh_ljs_undefined();
  SHLegacyValue np6 = _sh_ljs_undefined();

L0:
  ;
  locals.t1 = _sh_ljs_param(frame, 2);
  np2 = _sh_ljs_param(frame, 1);
  locals.t0 = _sh_ljs_get_env_from_closure(shr, frame[-7]);  np3 = _sh_ljs_load_from_env(locals.t0, 4);
  np0 = _sh_ljs_double(3);
  np1 = _sh_ljs_double(2);
  np4 = _sh_ljs_bool(_sh_ljs_get_double(np3) == _sh_ljs_get_double(np1));
  if(_sh_ljs_get_bool(np4)) goto L21;
  goto L1;

L1:
  ;
  np3 = _sh_ljs_bool(_sh_ljs_get_double(np3) == _sh_ljs_get_double(np0));
  if(_sh_ljs_get_bool(np3)) goto L16;
  goto L2;

L2:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np1, 4);
  np6 = _sh_ljs_load_from_env(locals.t0, 3);
  goto L3;
L3:
  ;
  np3 = _sh_ljs_double(1);
  np5 = _sh_ljs_bool(_sh_ljs_get_double(np2) == _sh_ljs_get_double(np3));
  np4 = _sh_ljs_double(0);
  np6 = _sh_ljs_bool(_sh_ljs_get_double(np4) == _sh_ljs_get_double(np6));
  if(_sh_ljs_get_bool(np6)) goto L9;
  goto L4;

L4:
  ;
  if(_sh_ljs_get_bool(np5)) goto L8;
  goto L5;

L5:
  ;
  np6 = _sh_ljs_bool(_sh_ljs_get_double(np2) == _sh_ljs_get_double(np1));
  if(_sh_ljs_get_bool(np6)) goto L7;
  goto L6;

L6:
  ;
  locals.t2 = _sh_ljs_load_from_env(locals.t0, 1);
  np6 = _sh_ljs_load_from_env(locals.t2, 1);
  np6 = _sh_ljs_double(_sh_ljs_get_double(np6) + _sh_ljs_get_double(np3));
  _sh_ljs_store_to_env(shr, locals.t2,np6, 1);
  np6 = _sh_ljs_load_from_env(locals.t2, 1);
  locals.t2 = _sh_ljs_load_from_env(locals.t2, 0);
  np6 = _sh_ljs_bool(_sh_ljs_less_rjs(shr, &np6, &locals.t2));
  if(_sh_ljs_get_bool(np6)) goto L13;
  goto L12;

L7:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  locals.t2 = _sh_ljs_new_object_with_buffer(shr, shUnit, 2, 1);
  _sh_prstore(shr, &locals.t2, 0, &locals.t1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t2;

L8:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  _sh_throw(shr, locals.t1);

L9:
  ;
  if(_sh_ljs_get_bool(np5)) goto L15;
  goto L10;

L10:
  ;
  np5 = _sh_ljs_bool(_sh_ljs_get_double(np2) == _sh_ljs_get_double(np1));
  if(_sh_ljs_get_bool(np5)) goto L14;
  goto L11;

L11:
  ;
  locals.t2 = _sh_ljs_get_env(shr, locals.t0, 1);
  _sh_ljs_store_to_env(shr, locals.t0,locals.t2, 2);
  locals.t3 = _sh_ljs_create_environment(shr, &locals.t2, 2);
  _sh_ljs_store_to_env(shr, locals.t0,locals.t3, 1);
  locals.t2 = _sh_ljs_load_from_env(locals.t0, 0);
  _sh_ljs_store_to_env(shr, locals.t3,locals.t2, 0);
  _sh_ljs_store_to_env(shr, locals.t3,np4, 1);
  np4 = _sh_ljs_load_from_env(locals.t3, 1);
  np4 = _sh_ljs_bool(_sh_ljs_less_rjs(shr, &np4, &locals.t2));
  if(_sh_ljs_get_bool(np4)) goto L13;
  goto L12;

L12:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  locals.t2 = _sh_ljs_new_object_with_buffer(shr, shUnit, 2, 1);
  np4 = _sh_ljs_undefined();
  _sh_prstore(shr, &locals.t2, 0, &np4);
  _sh_leave(shr, &locals.head, frame);
  return locals.t2;

L13:
  ;
  locals.t2 = _sh_ljs_load_from_env(locals.t0, 2);
  frame[3] = _sh_ljs_load_from_env(locals.t2, 1);
  np4 = _sh_ljs_undefined();
  frame[4] = _sh_ljs_undefined();
  frame[2] = _sh_ljs_undefined();
  locals.t3 = _sh_ljs_call(shr, frame, 0);
  _sh_ljs_store_to_env(shr, locals.t0,np3, 3);
  _sh_ljs_store_to_env(shr, locals.t0,np3, 4);
  locals.t2 = _sh_ljs_new_object_with_buffer(shr, shUnit, 2, 4);
  _sh_prstore(shr, &locals.t2, 0, &locals.t3);
  _sh_leave(shr, &locals.head, frame);
  return locals.t2;

L14:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  locals.t2 = _sh_ljs_new_object_with_buffer(shr, shUnit, 2, 1);
  _sh_prstore(shr, &locals.t2, 0, &locals.t1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t2;

L15:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  _sh_throw(shr, locals.t1);

L16:
  ;
  np3 = _sh_ljs_double(1);
  np3 = _sh_ljs_bool(_sh_ljs_get_double(np2) == _sh_ljs_get_double(np3));
  if(_sh_ljs_get_bool(np3)) goto L20;
  goto L17;

L17:
  ;
  locals.t2 = _sh_ljs_new_object_with_buffer(shr, shUnit, 2, 1);
  np1 = _sh_ljs_bool(_sh_ljs_get_double(np2) == _sh_ljs_get_double(np1));
  if(_sh_ljs_get_bool(np1)) goto L19;
  goto L18;

L18:
  ;
  np1 = _sh_ljs_undefined();
  _sh_prstore(shr, &locals.t2, 0, &np1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t2;

L19:
  ;
  _sh_prstore(shr, &locals.t2, 0, &locals.t1);
  _sh_leave(shr, &locals.head, frame);
  return locals.t2;

L20:
  ;
  _sh_throw(shr, locals.t1);

L21:
  ;
  _sh_ljs_store_to_env(shr, locals.t0,np0, 4);
  locals.t0 = _sh_ljs_get_string(shr, get_symbols(shUnit)[30] /*Generator functions ...*/);
  _sh_throw_type_error(shr, &locals.t0);
}
static unsigned char s_literal_val_buffer[6] = {97,1,17,2,1,33,};
static unsigned char s_obj_key_buffer[8] = {97,0,98,2,3,98,4,5,};
static const SHShapeTableEntry s_obj_shape_table[] = {
  { .key_buffer_offset = 0, .num_props = 1 },
  { .key_buffer_offset = 2, .num_props = 2 },
  { .key_buffer_offset = 5, .num_props = 2 },
};

static const SHSrcLoc s_source_locations[] = {
  { .filename_idx = 6, .line = 0, .column = 0 },
};

static SHNativeFuncInfo s_function_info_table[] = {
  { .name_index = 31, .arg_count = 0, .prohibit_invoke = 2, .kind = 0 },
  { .name_index = 7, .arg_count = 0, .prohibit_invoke = 2, .kind = 0 },
  { .name_index = 8, .arg_count = 0, .prohibit_invoke = 2, .kind = 0 },
  { .name_index = 32, .arg_count = 0, .prohibit_invoke = 2, .kind = 0 },
  { .name_index = 33, .arg_count = 0, .prohibit_invoke = 0, .kind = 0 },
  { .name_index = 0, .arg_count = 0, .prohibit_invoke = 1, .kind = 0 },
  { .name_index = 34, .arg_count = 0, .prohibit_invoke = 0, .kind = 0 },
  { .name_index = 20, .arg_count = 0, .prohibit_invoke = 1, .kind = 0 },
  { .name_index = 3, .arg_count = 1, .prohibit_invoke = 1, .kind = 1 },
  { .name_index = 2, .arg_count = 0, .prohibit_invoke = 1, .kind = 0 },
  { .name_index = 35, .arg_count = 1, .prohibit_invoke = 2, .kind = 0 },
};
static const char s_ascii_pool[] = {
  'f', 'i', 'r', 's', 't', '\0',
  'I', ' ', 'a', 'm', ' ', 'i', 'n', ' ', 't', 'h', 'e', ' ', 'p', 'r', 'o', 't', 'o', 't', 'y', 'p', 'e', '\0',
  'i', 'n', 'c', 'r', 'e', 'm', 'e', 'n', 't', '\0',
  'd', 'o', 'S', 't', 'e', 'p', 's', '\0',
  'v', 'a', 'l', 'u', 'e', '\0',
  'd', 'o', 'n', 'e', '\0',
  '\0',
  'c', 'r', 'e', 'a', 't', 'e', 'C', 'o', 'u', 'n', 't', 'e', 'r', 'W', 'i', 't', 'h', 'G', 'e', 'n', 'e', 'r', 'a', 't', 'o', 'r', '\0',
  's', 'h', 'o', 'w', '_', 't', 'd', 'z', '\0',
  'c', 'o', 'n', 's', 'o', 'l', 'e', '\0',
  'l', 'o', 'g', '\0',
  'C', 'l', 'o', 's', 'u', 'r', 'e', 's', '\x000A', '=', '=', '=', '=', '=', '=', '=', '=', '=', '\0',
  'H', 'e', 'r', 'm', 'e', 's', 'I', 'n', 't', 'e', 'r', 'n', 'a', 'l', '\0',
  'c', 'o', 'n', 'c', 'a', 't', '\0',
  'G', 'e', 'n', 'e', 'r', 'a', 't', 'i', 'n', 'g', ' ', '\0',
  ' ', 'i', 'n', 'c', 'r', 'e', 'm', 'e', 'n', 't', 's', ' ', 'w', 'i', 't', 'h', ' ', 'd', 'e', 'f', 'a', 'u', 'l', 't', ' ', 's', 't', 'e', 'p', ' ', 's', 'i', 'z', 'e', ':', '\0',
  'F', 'u', 'r', 't', 'h', 'e', 'r', ' ', 'i', 'n', 'c', 'r', 'e', 'm', 'e', 'n', 't', 's', ':', '\0',
  '\x000A', 'T', 'D', 'Z', '\x000A', '=', '=', '=', '=', '=', '=', '=', '=', '=', '\0',
  's', 't', 'a', 'c', 'k', '\0',
  'T', 'D', 'Z', ' ', 'E', 'r', 'r', 'o', 'r', '!', '\0',
  's', 'e', 'c', 'o', 'n', 'd', '\0',
  '\x000A', 'P', 'r', 'o', 't', 'o', 't', 'y', 'p', 'i', 'c', 'a', 'l', ' ', 'I', 'n', 'h', 'e', 'r', 'i', 't', 'a', 'n', 'c', 'e', '\x000A', '=', '=', '=', '=', '=', '=', '=', '=', '=', '\0',
  'F', 'i', 'r', 's', 't', ' ', 'p', 'r', 'o', 'p', 'e', 'r', 't', 'y', ':', '\0',
  'S', 'e', 'c', 'o', 'n', 'd', ' ', 'p', 'r', 'o', 'p', 'e', 'r', 't', 'y', ':', '\0',
  't', 'h', 'i', 'r', 'd', '\0',
  'T', 'h', 'i', 'r', 'd', ' ', 'p', 'r', 'o', 'p', 'e', 'r', 't', 'y', ':', '\0',
  'S', 'e', 'c', 'o', 'n', 'd', ' ', 'p', 'r', 'o', 'p', 'e', 'r', 't', 'y', ' ', 'a', 'g', 'a', 'i', 'n', ':', '\0',
  'T', 'h', 'i', 'r', 'd', ' ', 'p', 'r', 'o', 'p', 'e', 'r', 't', 'y', ' ', 'n', 'o', 'w', ':', '\0',
  '\x000A', 'C', 'l', 'a', 's', 's', 'e', 's', '\x000A', '=', '=', '=', '=', '=', '=', '=', '=', '=', '\0',
  'G', 'e', 't', 't', 'e', 'r', ' ', 'e', 'x', 'e', 'c', 'u', 't', 'e', 'd', ',', ' ', 't', 'h', 'i', 'r', 'd', ' ', 'i', 's', ' ', 'n', 'o', 'w', ' ', '\0',
  'G', 'e', 'n', 'e', 'r', 'a', 't', 'o', 'r', ' ', 'f', 'u', 'n', 'c', 't', 'i', 'o', 'n', 's', ' ', 'm', 'a', 'y', ' ', 'n', 'o', 't', ' ', 'b', 'e', ' ', 'c', 'a', 'l', 'l', 'e', 'd', ' ', 'o', 'n', ' ', 'e', 'x', 'e', 'c', 'u', 't', 'i', 'n', 'g', ' ', 'g', 'e', 'n', 'e', 'r', 'a', 't', 'o', 'r', 's', '\0',
  'g', 'l', 'o', 'b', 'a', 'l', '\0',
  'g', 'e', 't', ' ', 's', 'e', 'c', 'o', 'n', 'd', '\0',
  'P', 'r', 'o', 't', 'o', 't', 'y', 'p', 'e', 'C', 'l', 'a', 's', 's', '\0',
  'D', 'e', 'r', 'i', 'v', 'e', 'd', 'C', 'l', 'a', 's', 's', '\0',
  'd', 'o', 'S', 't', 'e', 'p', 's', '?', 'i', 'n', 'n', 'e', 'r', '\0',
};
static const char16_t s_u16_pool[] = {
};
static const uint32_t s_strings[] = {0,5,1277812307,6,21,1129390002,28,9,615959742,38,7,3443187243,46,5,3746588989,52,4,3042174909,57,0,0,58,26,1379064149,85,8,3278253158,94,7,1654270973,102,3,473294856,106,18,1728543097,125,14,2243688185,140,6,3415079525,147,11,2920213895,159,35,1982628859,195,19,890115180,215,14,3878896930,230,5,2203018044,236,10,4289218586,247,6,642931206,254,35,568872,290,15,306063864,306,16,1291038032,323,5,3009989046,329,15,3967019174,345,22,2290593654,368,19,2289934661,388,18,1555715664,407,30,2959272151,438,61,632917851,500,6,615793799,507,10,3810735167,518,14,687067724,533,12,1727216164,546,13,2677734050,};
#define CREATE_THIS_UNIT sh_export_this_unit
struct UnitData {
  SHUnit unit;
  SHSymbolID symbol_data[36];
  SHPropertyCacheEntry prop_cache_data[72];
;  SHCompressedPointer object_literal_class_cache[3];
};
SHUnit *CREATE_THIS_UNIT(void) {
  struct UnitData *unit_data = calloc(sizeof(struct UnitData), 1);
  *unit_data = (struct UnitData){.unit = {.index = &unit_index,.num_symbols =36, .num_prop_cache_entries = 72, .ascii_pool = s_ascii_pool, .u16_pool = s_u16_pool,.strings = s_strings, .symbols = unit_data->symbol_data,.prop_cache = unit_data->prop_cache_data,.obj_key_buffer = s_obj_key_buffer, .obj_key_buffer_size = 8, .literal_val_buffer = s_literal_val_buffer, .literal_val_buffer_size = 6, .obj_shape_table = s_obj_shape_table, .obj_shape_table_count = 3, .object_literal_class_cache = unit_data->object_literal_class_cache, .source_locations = s_source_locations, .source_locations_size = 1, .unit_main = _0_global, .unit_main_info = &s_function_info_table[0], .unit_name = "sh_compiled" }};
  return (SHUnit *)unit_data;
}

SHSymbolID *get_symbols(SHUnit *unit) {
  return ((struct UnitData *)unit)->symbol_data;
}

SHPropertyCacheEntry *get_prop_cache(SHUnit *unit) {
  return ((struct UnitData *)unit)->prop_cache_data;
}

void init_console_bindings(SHRuntime *shr);

int main(int argc, char **argv) {
  SHRuntime *shr = _sh_init(argc, argv);
  init_console_bindings(shr);
  bool success = _sh_initialize_units(shr, 1, CREATE_THIS_UNIT);
  _sh_done(shr);
  return success ? 0 : 1;
}
