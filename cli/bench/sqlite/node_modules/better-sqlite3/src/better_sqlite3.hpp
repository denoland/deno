// better_sqlite3.hpp
//

#ifndef LZZ_BETTER_SQLITE3_better_sqlite3_hpp
#define LZZ_BETTER_SQLITE3_better_sqlite3_hpp
#line 2 "./src/better_sqlite3.lzz"
#include <climits>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include <set>
#include <unordered_map>
#include <algorithm>
#include <sqlite3.h>
#include <node.h>
#include <node_object_wrap.h>
#include <node_buffer.h>
#line 31 "./src/util/macros.lzz"
template <class T> using CopyablePersistent = v8::Persistent<T, v8::CopyablePersistentTraits<T>>;
#define LZZ_INLINE inline
#line 16 "./src/util/macros.lzz"
v8::Local <v8::String> StringFromUtf8 (v8::Isolate * isolate, char const * data, int length);
#line 19 "./src/util/macros.lzz"
v8::Local <v8::String> InternalizedFromUtf8 (v8::Isolate * isolate, char const * data, int length);
#line 22 "./src/util/macros.lzz"
v8::Local <v8::Value> InternalizedFromUtf8OrNull (v8::Isolate * isolate, char const * data, int length);
#line 26 "./src/util/macros.lzz"
v8::Local <v8::String> InternalizedFromLatin1 (v8::Isolate * isolate, char const * str);
#line 33 "./src/util/macros.lzz"
void SetFrozen (v8::Isolate * isolate, v8::Local <v8::Context> ctx, v8::Local <v8::Object> obj, CopyablePersistent <v8::String> & key, v8::Local <v8::Value> value);
#line 37 "./src/util/macros.lzz"
void ThrowError (char const * message);
#line 38 "./src/util/macros.lzz"
void ThrowTypeError (char const * message);
#line 39 "./src/util/macros.lzz"
void ThrowRangeError (char const * message);
#line 91 "./src/util/macros.lzz"
bool IS_SKIPPED (char c);
#line 96 "./src/util/macros.lzz"
template <typename T>
#line 96 "./src/util/macros.lzz"
T * ALLOC_ARRAY (size_t count);
#line 101 "./src/util/macros.lzz"
template <typename T>
#line 101 "./src/util/macros.lzz"
void FREE_ARRAY (T * array_pointer);
#line 105 "./src/util/macros.lzz"
v8::Local <v8::FunctionTemplate> NewConstructorTemplate (v8::Isolate * isolate, v8::Local <v8::External> data, v8::FunctionCallback func, char const * name);
#line 116 "./src/util/macros.lzz"
void SetPrototypeMethod (v8::Isolate * isolate, v8::Local <v8::External> data, v8::Local <v8::FunctionTemplate> recv, char const * name, v8::FunctionCallback func);
#line 129 "./src/util/macros.lzz"
void SetPrototypeSymbolMethod (v8::Isolate * isolate, v8::Local <v8::External> data, v8::Local <v8::FunctionTemplate> recv, v8::Local <v8::Symbol> symbol, v8::FunctionCallback func);
#line 142 "./src/util/macros.lzz"
void SetPrototypeGetter (v8::Isolate * isolate, v8::Local <v8::External> data, v8::Local <v8::FunctionTemplate> recv, char const * name, v8::AccessorGetterCallback func);
#line 1 "./src/util/constants.lzz"
class CS
{
#line 2 "./src/util/constants.lzz"
public:
#line 4 "./src/util/constants.lzz"
  v8::Local <v8::String> Code (v8::Isolate * isolate, int code);
#line 10 "./src/util/constants.lzz"
  explicit CS (v8::Isolate * isolate);
#line 119 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> database;
#line 120 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> reader;
#line 121 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> source;
#line 122 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> memory;
#line 123 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> readonly;
#line 124 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> name;
#line 125 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> next;
#line 126 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> length;
#line 127 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> done;
#line 128 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> value;
#line 129 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> changes;
#line 130 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> lastInsertRowid;
#line 131 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> statement;
#line 132 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> column;
#line 133 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> table;
#line 134 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> type;
#line 135 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> totalPages;
#line 136 "./src/util/constants.lzz"
  CopyablePersistent <v8::String> remainingPages;
#line 138 "./src/util/constants.lzz"
private:
#line 140 "./src/util/constants.lzz"
  static void SetString (v8::Isolate * isolate, CopyablePersistent <v8::String> & constant, char const * str);
#line 144 "./src/util/constants.lzz"
  void SetCode (v8::Isolate * isolate, int code, char const * str);
#line 150 "./src/util/constants.lzz"
  std::unordered_map <int, CopyablePersistent<v8::String> > codes;
};
#line 1 "./src/util/bind-map.lzz"
class BindMap
{
#line 2 "./src/util/bind-map.lzz"
public:
#line 6 "./src/util/bind-map.lzz"
  class Pair
  {
#line 6 "./src/util/bind-map.lzz"
    friend class BindMap;
#line 7 "./src/util/bind-map.lzz"
  public:
#line 9 "./src/util/bind-map.lzz"
    int GetIndex ();
#line 13 "./src/util/bind-map.lzz"
    v8::Local <v8::String> GetName (v8::Isolate * isolate);
#line 17 "./src/util/bind-map.lzz"
  private:
#line 19 "./src/util/bind-map.lzz"
    explicit Pair (v8::Isolate * isolate, char const * name, int index);
#line 22 "./src/util/bind-map.lzz"
    explicit Pair (v8::Isolate * isolate, Pair * pair);
#line 25 "./src/util/bind-map.lzz"
    CopyablePersistent <v8::String> const name;
#line 26 "./src/util/bind-map.lzz"
    int const index;
  };
#line 29 "./src/util/bind-map.lzz"
  explicit BindMap (char _);
#line 36 "./src/util/bind-map.lzz"
  ~ BindMap ();
#line 41 "./src/util/bind-map.lzz"
  Pair * GetPairs ();
#line 45 "./src/util/bind-map.lzz"
  int GetSize ();
#line 50 "./src/util/bind-map.lzz"
  void Add (v8::Isolate * isolate, char const * name, int index);
#line 56 "./src/util/bind-map.lzz"
private:
#line 58 "./src/util/bind-map.lzz"
  void Grow (v8::Isolate * isolate);
#line 70 "./src/util/bind-map.lzz"
  Pair * pairs;
#line 71 "./src/util/bind-map.lzz"
  int capacity;
#line 72 "./src/util/bind-map.lzz"
  int length;
};
#line 20 "./src/better_sqlite3.lzz"
struct Addon;
#line 21 "./src/better_sqlite3.lzz"
class Statement;
#line 22 "./src/better_sqlite3.lzz"
class Backup;
#line 1 "./src/objects/database.lzz"
class Database : public node::ObjectWrap
{
#line 2 "./src/objects/database.lzz"
public:
#line 4 "./src/objects/database.lzz"
  static v8::Local <v8 :: Function> Init (v8::Isolate * isolate, v8::Local <v8 :: External> data);
#line 23 "./src/objects/database.lzz"
  class CompareDatabase
  {
#line 23 "./src/objects/database.lzz"
  public:
#line 24 "./src/objects/database.lzz"
    bool operator () (Database const * const a, Database const * const b) const;
  };
#line 28 "./src/objects/database.lzz"
  class CompareStatement
  {
#line 28 "./src/objects/database.lzz"
  public:
#line 29 "./src/objects/database.lzz"
    bool operator () (Statement const * const a, Statement const * const b) const;
  };
#line 33 "./src/objects/database.lzz"
  class CompareBackup
  {
#line 33 "./src/objects/database.lzz"
  public:
#line 34 "./src/objects/database.lzz"
    bool operator () (Backup const * const a, Backup const * const b) const;
  };
#line 40 "./src/objects/database.lzz"
  void ThrowDatabaseError ();
#line 44 "./src/objects/database.lzz"
  static void ThrowSqliteError (Addon * addon, sqlite3 * db_handle);
#line 48 "./src/objects/database.lzz"
  static void ThrowSqliteError (Addon * addon, char const * message, int code);
#line 64 "./src/objects/database.lzz"
  bool Log (v8::Isolate * isolate, sqlite3_stmt * handle);
#line 77 "./src/objects/database.lzz"
  void AddStatement (Statement * stmt);
#line 78 "./src/objects/database.lzz"
  void RemoveStatement (Statement * stmt);
#line 81 "./src/objects/database.lzz"
  void AddBackup (Backup * backup);
#line 82 "./src/objects/database.lzz"
  void RemoveBackup (Backup * backup);
#line 86 "./src/objects/database.lzz"
  struct State
  {
#line 87 "./src/objects/database.lzz"
    bool const open;
#line 88 "./src/objects/database.lzz"
    bool busy;
#line 89 "./src/objects/database.lzz"
    bool const safe_ints;
#line 90 "./src/objects/database.lzz"
    bool const unsafe_mode;
#line 91 "./src/objects/database.lzz"
    bool was_js_error;
#line 92 "./src/objects/database.lzz"
    bool const has_logger;
#line 93 "./src/objects/database.lzz"
    unsigned short int iterators;
#line 94 "./src/objects/database.lzz"
    Addon * const addon;
  };
#line 96 "./src/objects/database.lzz"
  State * GetState ();
#line 99 "./src/objects/database.lzz"
  sqlite3 * GetHandle ();
#line 102 "./src/objects/database.lzz"
  Addon * GetAddon ();
#line 107 "./src/objects/database.lzz"
  void CloseHandles ();
#line 119 "./src/objects/database.lzz"
  ~ Database ();
#line 124 "./src/objects/database.lzz"
private:
#line 126 "./src/objects/database.lzz"
  explicit Database (v8::Isolate * isolate, Addon * addon, sqlite3 * db_handle, v8::Local <v8::Value> logger);
#line 149 "./src/objects/database.lzz"
  static void JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 201 "./src/objects/database.lzz"
  static void JS_prepare (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 217 "./src/objects/database.lzz"
  static void JS_exec (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 257 "./src/objects/database.lzz"
  static void JS_backup (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 275 "./src/objects/database.lzz"
  static void JS_serialize (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 297 "./src/objects/database.lzz"
  static void JS_function (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 321 "./src/objects/database.lzz"
  static void JS_aggregate (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 350 "./src/objects/database.lzz"
  static void JS_table (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 370 "./src/objects/database.lzz"
  static void JS_loadExtension (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 392 "./src/objects/database.lzz"
  static void JS_close (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 402 "./src/objects/database.lzz"
  static void JS_defaultSafeIntegers (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 408 "./src/objects/database.lzz"
  static void JS_unsafeMode (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 415 "./src/objects/database.lzz"
  static void JS_open (v8::Local <v8 :: String> _, v8::PropertyCallbackInfo <v8 :: Value> const & info);
#line 419 "./src/objects/database.lzz"
  static void JS_inTransaction (v8::Local <v8 :: String> _, v8::PropertyCallbackInfo <v8 :: Value> const & info);
#line 424 "./src/objects/database.lzz"
  static bool Deserialize (v8::Local <v8::Object> buffer, Addon * addon, sqlite3 * db_handle, bool readonly);
#line 449 "./src/objects/database.lzz"
  static void FreeSerialization (char * data, void * _);
#line 453 "./src/objects/database.lzz"
  static int const MAX_BUFFER_SIZE = node::Buffer::kMaxLength > INT_MAX ? INT_MAX : static_cast<int>(node::Buffer::kMaxLength);
#line 454 "./src/objects/database.lzz"
  static int const MAX_STRING_SIZE = v8::String::kMaxLength > INT_MAX ? INT_MAX : static_cast<int>(v8::String::kMaxLength);
#line 456 "./src/objects/database.lzz"
  sqlite3 * const db_handle;
#line 457 "./src/objects/database.lzz"
  bool open;
#line 458 "./src/objects/database.lzz"
  bool busy;
#line 459 "./src/objects/database.lzz"
  bool safe_ints;
#line 460 "./src/objects/database.lzz"
  bool unsafe_mode;
#line 461 "./src/objects/database.lzz"
  bool was_js_error;
#line 462 "./src/objects/database.lzz"
  bool const has_logger;
#line 463 "./src/objects/database.lzz"
  unsigned short int iterators;
#line 464 "./src/objects/database.lzz"
  Addon * const addon;
#line 465 "./src/objects/database.lzz"
  CopyablePersistent <v8::Value> const logger;
#line 466 "./src/objects/database.lzz"
  std::set <Statement*, CompareStatement> stmts;
#line 467 "./src/objects/database.lzz"
  std::set <Backup*, CompareBackup> backups;
};
#line 1 "./src/objects/statement.lzz"
class Statement : public node::ObjectWrap
{
#line 1 "./src/objects/statement.lzz"
  friend class StatementIterator;
#line 2 "./src/objects/statement.lzz"
public:
#line 4 "./src/objects/statement.lzz"
  static v8::Local <v8 :: Function> Init (v8::Isolate * isolate, v8::Local <v8 :: External> data);
#line 21 "./src/objects/statement.lzz"
  static bool Compare (Statement const * const a, Statement const * const b);
#line 26 "./src/objects/statement.lzz"
  BindMap * GetBindMap (v8::Isolate * isolate);
#line 39 "./src/objects/statement.lzz"
  void CloseHandles ();
#line 46 "./src/objects/statement.lzz"
  ~ Statement ();
#line 52 "./src/objects/statement.lzz"
private:
#line 55 "./src/objects/statement.lzz"
  class Extras
  {
#line 55 "./src/objects/statement.lzz"
    friend class Statement;
#line 56 "./src/objects/statement.lzz"
    explicit Extras (sqlite3_uint64 id);
#line 57 "./src/objects/statement.lzz"
    BindMap bind_map;
#line 58 "./src/objects/statement.lzz"
    sqlite3_uint64 const id;
  };
#line 61 "./src/objects/statement.lzz"
  explicit Statement (Database * db, sqlite3_stmt * handle, sqlite3_uint64 id, bool returns_data);
#line 85 "./src/objects/statement.lzz"
  static void JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 149 "./src/objects/statement.lzz"
  static void JS_run (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 172 "./src/objects/statement.lzz"
  static void JS_get (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 187 "./src/objects/statement.lzz"
  static void JS_all (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 208 "./src/objects/statement.lzz"
  static void JS_iterate (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 218 "./src/objects/statement.lzz"
  static void JS_bind (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 229 "./src/objects/statement.lzz"
  static void JS_pluck (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 240 "./src/objects/statement.lzz"
  static void JS_expand (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 251 "./src/objects/statement.lzz"
  static void JS_raw (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 262 "./src/objects/statement.lzz"
  static void JS_safeIntegers (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 271 "./src/objects/statement.lzz"
  static void JS_columns (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 314 "./src/objects/statement.lzz"
  static void JS_busy (v8::Local <v8 :: String> _, v8::PropertyCallbackInfo <v8 :: Value> const & info);
#line 319 "./src/objects/statement.lzz"
  Database * const db;
#line 320 "./src/objects/statement.lzz"
  sqlite3_stmt * const handle;
#line 321 "./src/objects/statement.lzz"
  Extras * const extras;
#line 322 "./src/objects/statement.lzz"
  bool alive;
#line 323 "./src/objects/statement.lzz"
  bool locked;
#line 324 "./src/objects/statement.lzz"
  bool bound;
#line 325 "./src/objects/statement.lzz"
  bool has_bind_map;
#line 326 "./src/objects/statement.lzz"
  bool safe_ints;
#line 327 "./src/objects/statement.lzz"
  char mode;
#line 328 "./src/objects/statement.lzz"
  bool const returns_data;
};
#line 1 "./src/objects/statement-iterator.lzz"
class StatementIterator : public node::ObjectWrap
{
#line 2 "./src/objects/statement-iterator.lzz"
public:
#line 4 "./src/objects/statement-iterator.lzz"
  static v8::Local <v8 :: Function> Init (v8::Isolate * isolate, v8::Local <v8 :: External> data);
#line 15 "./src/objects/statement-iterator.lzz"
  ~ StatementIterator ();
#line 17 "./src/objects/statement-iterator.lzz"
private:
#line 19 "./src/objects/statement-iterator.lzz"
  explicit StatementIterator (Statement * stmt, bool bound);
#line 38 "./src/objects/statement-iterator.lzz"
  static void JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 57 "./src/objects/statement-iterator.lzz"
  static void JS_next (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 64 "./src/objects/statement-iterator.lzz"
  static void JS_return (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 71 "./src/objects/statement-iterator.lzz"
  static void JS_symbolIterator (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 75 "./src/objects/statement-iterator.lzz"
  void Next (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 100 "./src/objects/statement-iterator.lzz"
  void Return (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 105 "./src/objects/statement-iterator.lzz"
  void Throw ();
#line 111 "./src/objects/statement-iterator.lzz"
  void Cleanup ();
#line 119 "./src/objects/statement-iterator.lzz"
  static v8::Local <v8::Object> NewRecord (v8::Isolate * isolate, v8::Local <v8::Context> ctx, v8::Local <v8::Value> value, Addon * addon, bool done);
#line 126 "./src/objects/statement-iterator.lzz"
  static v8::Local <v8::Object> DoneRecord (v8::Isolate * isolate, Addon * addon);
#line 130 "./src/objects/statement-iterator.lzz"
  Statement * const stmt;
#line 131 "./src/objects/statement-iterator.lzz"
  sqlite3_stmt * const handle;
#line 132 "./src/objects/statement-iterator.lzz"
  Database::State * const db_state;
#line 133 "./src/objects/statement-iterator.lzz"
  bool const bound;
#line 134 "./src/objects/statement-iterator.lzz"
  bool const safe_ints;
#line 135 "./src/objects/statement-iterator.lzz"
  char const mode;
#line 136 "./src/objects/statement-iterator.lzz"
  bool alive;
#line 137 "./src/objects/statement-iterator.lzz"
  bool logged;
};
#line 1 "./src/objects/backup.lzz"
class Backup : public node::ObjectWrap
{
#line 2 "./src/objects/backup.lzz"
public:
#line 4 "./src/objects/backup.lzz"
  static v8::Local <v8 :: Function> Init (v8::Isolate * isolate, v8::Local <v8 :: External> data);
#line 12 "./src/objects/backup.lzz"
  static bool Compare (Backup const * const a, Backup const * const b);
#line 17 "./src/objects/backup.lzz"
  void CloseHandles ();
#line 28 "./src/objects/backup.lzz"
  ~ Backup ();
#line 33 "./src/objects/backup.lzz"
private:
#line 35 "./src/objects/backup.lzz"
  explicit Backup (Database * db, sqlite3 * dest_handle, sqlite3_backup * backup_handle, sqlite3_uint64 id, bool unlink);
#line 55 "./src/objects/backup.lzz"
  static void JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 98 "./src/objects/backup.lzz"
  static void JS_transfer (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 124 "./src/objects/backup.lzz"
  static void JS_close (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 132 "./src/objects/backup.lzz"
  Database * const db;
#line 133 "./src/objects/backup.lzz"
  sqlite3 * const dest_handle;
#line 134 "./src/objects/backup.lzz"
  sqlite3_backup * const backup_handle;
#line 135 "./src/objects/backup.lzz"
  sqlite3_uint64 const id;
#line 136 "./src/objects/backup.lzz"
  bool alive;
#line 137 "./src/objects/backup.lzz"
  bool unlink;
};
#line 1 "./src/util/data-converter.lzz"
class DataConverter
{
#line 2 "./src/util/data-converter.lzz"
public:
#line 4 "./src/util/data-converter.lzz"
  void ThrowDataConversionError (sqlite3_context * invocation, bool isBigInt);
#line 13 "./src/util/data-converter.lzz"
protected:
#line 15 "./src/util/data-converter.lzz"
  virtual void PropagateJSError (sqlite3_context * invocation) = 0;
#line 16 "./src/util/data-converter.lzz"
  virtual std::string GetDataErrorPrefix () = 0;
};
#line 1 "./src/util/custom-function.lzz"
class CustomFunction : protected DataConverter
{
#line 2 "./src/util/custom-function.lzz"
public:
#line 4 "./src/util/custom-function.lzz"
  explicit CustomFunction (v8::Isolate * isolate, Database * db, char const * name, v8::Local <v8::Function> fn, bool safe_ints);
#line 17 "./src/util/custom-function.lzz"
  virtual ~ CustomFunction ();
#line 19 "./src/util/custom-function.lzz"
  static void xDestroy (void * self);
#line 23 "./src/util/custom-function.lzz"
  static void xFunc (sqlite3_context * invocation, int argc, sqlite3_value * * argv);
#line 40 "./src/util/custom-function.lzz"
protected:
#line 42 "./src/util/custom-function.lzz"
  void PropagateJSError (sqlite3_context * invocation);
#line 48 "./src/util/custom-function.lzz"
  std::string GetDataErrorPrefix ();
#line 52 "./src/util/custom-function.lzz"
private:
#line 53 "./src/util/custom-function.lzz"
  std::string const name;
#line 54 "./src/util/custom-function.lzz"
  Database * const db;
#line 55 "./src/util/custom-function.lzz"
protected:
#line 56 "./src/util/custom-function.lzz"
  v8::Isolate * const isolate;
#line 57 "./src/util/custom-function.lzz"
  CopyablePersistent <v8::Function> const fn;
#line 58 "./src/util/custom-function.lzz"
  bool const safe_ints;
};
#line 1 "./src/util/custom-aggregate.lzz"
class CustomAggregate : public CustomFunction
{
#line 2 "./src/util/custom-aggregate.lzz"
public:
#line 4 "./src/util/custom-aggregate.lzz"
  explicit CustomAggregate (v8::Isolate * isolate, Database * db, char const * name, v8::Local <v8::Value> start, v8::Local <v8::Function> step, v8::Local <v8::Value> inverse, v8::Local <v8::Value> result, bool safe_ints);
#line 21 "./src/util/custom-aggregate.lzz"
  static void xStep (sqlite3_context * invocation, int argc, sqlite3_value * * argv);
#line 25 "./src/util/custom-aggregate.lzz"
  static void xInverse (sqlite3_context * invocation, int argc, sqlite3_value * * argv);
#line 29 "./src/util/custom-aggregate.lzz"
  static void xValue (sqlite3_context * invocation);
#line 33 "./src/util/custom-aggregate.lzz"
  static void xFinal (sqlite3_context * invocation);
#line 37 "./src/util/custom-aggregate.lzz"
private:
#line 39 "./src/util/custom-aggregate.lzz"
  static void xStepBase (sqlite3_context * invocation, int argc, sqlite3_value * * argv, CopyablePersistent <v8::Function> const CustomAggregate::* ptrtm);
#line 58 "./src/util/custom-aggregate.lzz"
  static void xValueBase (sqlite3_context * invocation, bool is_final);
#line 82 "./src/util/custom-aggregate.lzz"
  struct Accumulator
  {
#line 82 "./src/util/custom-aggregate.lzz"
  public:
#line 83 "./src/util/custom-aggregate.lzz"
    CopyablePersistent <v8::Value> value;
#line 84 "./src/util/custom-aggregate.lzz"
    bool initialized;
#line 85 "./src/util/custom-aggregate.lzz"
    bool is_window;
  };
#line 88 "./src/util/custom-aggregate.lzz"
  Accumulator * GetAccumulator (sqlite3_context * invocation);
#line 105 "./src/util/custom-aggregate.lzz"
  static void DestroyAccumulator (sqlite3_context * invocation);
#line 111 "./src/util/custom-aggregate.lzz"
  void PropagateJSError (sqlite3_context * invocation);
#line 116 "./src/util/custom-aggregate.lzz"
  bool const invoke_result;
#line 117 "./src/util/custom-aggregate.lzz"
  bool const invoke_start;
#line 118 "./src/util/custom-aggregate.lzz"
  CopyablePersistent <v8::Function> const inverse;
#line 119 "./src/util/custom-aggregate.lzz"
  CopyablePersistent <v8::Function> const result;
#line 120 "./src/util/custom-aggregate.lzz"
  CopyablePersistent <v8::Value> const start;
};
#line 1 "./src/util/custom-table.lzz"
class CustomTable
{
#line 2 "./src/util/custom-table.lzz"
public:
#line 4 "./src/util/custom-table.lzz"
  explicit CustomTable (v8::Isolate * isolate, Database * db, char const * name, v8::Local <v8::Function> factory);
#line 16 "./src/util/custom-table.lzz"
  static void Destructor (void * self);
#line 20 "./src/util/custom-table.lzz"
  static sqlite3_module MODULE;
#line 47 "./src/util/custom-table.lzz"
  static sqlite3_module EPONYMOUS_MODULE;
#line 74 "./src/util/custom-table.lzz"
private:
#line 77 "./src/util/custom-table.lzz"
  class VTab
  {
#line 77 "./src/util/custom-table.lzz"
    friend class CustomTable;
#line 78 "./src/util/custom-table.lzz"
    explicit VTab (CustomTable * parent, v8::Local <v8::Function> generator, std::vector <std::string> parameter_names, bool safe_ints);
#line 92 "./src/util/custom-table.lzz"
    static CustomTable::VTab * Upcast (sqlite3_vtab * vtab);
#line 96 "./src/util/custom-table.lzz"
    sqlite3_vtab * Downcast ();
#line 100 "./src/util/custom-table.lzz"
    sqlite3_vtab base;
#line 101 "./src/util/custom-table.lzz"
    CustomTable * const parent;
#line 102 "./src/util/custom-table.lzz"
    int const parameter_count;
#line 103 "./src/util/custom-table.lzz"
    bool const safe_ints;
#line 104 "./src/util/custom-table.lzz"
    CopyablePersistent <v8::Function> const generator;
#line 105 "./src/util/custom-table.lzz"
    std::vector <std::string> const parameter_names;
  };
#line 109 "./src/util/custom-table.lzz"
  class Cursor
  {
#line 109 "./src/util/custom-table.lzz"
    friend class CustomTable;
#line 110 "./src/util/custom-table.lzz"
    static CustomTable::Cursor * Upcast (sqlite3_vtab_cursor * cursor);
#line 114 "./src/util/custom-table.lzz"
    sqlite3_vtab_cursor * Downcast ();
#line 118 "./src/util/custom-table.lzz"
    CustomTable::VTab * GetVTab ();
#line 122 "./src/util/custom-table.lzz"
    sqlite3_vtab_cursor base;
#line 123 "./src/util/custom-table.lzz"
    CopyablePersistent <v8::Object> iterator;
#line 124 "./src/util/custom-table.lzz"
    CopyablePersistent <v8::Function> next;
#line 125 "./src/util/custom-table.lzz"
    CopyablePersistent <v8::Array> row;
#line 126 "./src/util/custom-table.lzz"
    bool done;
#line 127 "./src/util/custom-table.lzz"
    sqlite_int64 rowid;
  };
#line 131 "./src/util/custom-table.lzz"
  class TempDataConverter : DataConverter
  {
#line 131 "./src/util/custom-table.lzz"
    friend class CustomTable;
#line 132 "./src/util/custom-table.lzz"
    explicit TempDataConverter (CustomTable * parent);
#line 136 "./src/util/custom-table.lzz"
    void PropagateJSError (sqlite3_context * invocation);
#line 141 "./src/util/custom-table.lzz"
    std::string GetDataErrorPrefix ();
#line 145 "./src/util/custom-table.lzz"
    CustomTable * const parent;
#line 146 "./src/util/custom-table.lzz"
    int status;
  };
#line 151 "./src/util/custom-table.lzz"
  static int xCreate (sqlite3 * db_handle, void * _self, int argc, char const * const * argv, sqlite3_vtab * * output, char * * errOutput);
#line 156 "./src/util/custom-table.lzz"
  static int xConnect (sqlite3 * db_handle, void * _self, int argc, char const * const * argv, sqlite3_vtab * * output, char * * errOutput);
#line 210 "./src/util/custom-table.lzz"
  static int xDisconnect (sqlite3_vtab * vtab);
#line 215 "./src/util/custom-table.lzz"
  static int xOpen (sqlite3_vtab * vtab, sqlite3_vtab_cursor * * output);
#line 220 "./src/util/custom-table.lzz"
  static int xClose (sqlite3_vtab_cursor * cursor);
#line 228 "./src/util/custom-table.lzz"
  static int xFilter (sqlite3_vtab_cursor * _cursor, int idxNum, char const * idxStr, int argc, sqlite3_value * * argv);
#line 284 "./src/util/custom-table.lzz"
  static int xNext (sqlite3_vtab_cursor * _cursor);
#line 313 "./src/util/custom-table.lzz"
  static int xEof (sqlite3_vtab_cursor * cursor);
#line 318 "./src/util/custom-table.lzz"
  static int xColumn (sqlite3_vtab_cursor * _cursor, sqlite3_context * invocation, int column);
#line 336 "./src/util/custom-table.lzz"
  static int xRowid (sqlite3_vtab_cursor * cursor, sqlite_int64 * output);
#line 343 "./src/util/custom-table.lzz"
  static int xBestIndex (sqlite3_vtab * vtab, sqlite3_index_info * output);
#line 387 "./src/util/custom-table.lzz"
  void PropagateJSError ();
#line 392 "./src/util/custom-table.lzz"
  Addon * const addon;
#line 393 "./src/util/custom-table.lzz"
  v8::Isolate * const isolate;
#line 394 "./src/util/custom-table.lzz"
  Database * const db;
#line 395 "./src/util/custom-table.lzz"
  std::string const name;
#line 396 "./src/util/custom-table.lzz"
  CopyablePersistent <v8::Function> const factory;
};
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 72 "./src/util/data.lzz"
  v8::Local <v8::Value> GetValueJS (v8::Isolate * isolate, sqlite3_stmt * handle, int column, bool safe_ints);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 76 "./src/util/data.lzz"
  v8::Local <v8::Value> GetValueJS (v8::Isolate * isolate, sqlite3_value * value, bool safe_ints);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 80 "./src/util/data.lzz"
  v8::Local <v8::Value> GetFlatRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 91 "./src/util/data.lzz"
  v8::Local <v8::Value> GetExpandedRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 110 "./src/util/data.lzz"
  v8::Local <v8::Value> GetRawRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 119 "./src/util/data.lzz"
  v8::Local <v8::Value> GetRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints, char mode);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 128 "./src/util/data.lzz"
  void GetArgumentsJS (v8::Isolate * isolate, v8::Local <v8::Value> * out, sqlite3_value * * values, int argument_count, bool safe_ints);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 135 "./src/util/data.lzz"
  int BindValueFromJS (v8::Isolate * isolate, sqlite3_stmt * handle, int index, v8::Local <v8::Value> value);
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 140 "./src/util/data.lzz"
  void ResultValueFromJS (v8::Isolate * isolate, sqlite3_context * invocation, v8::Local <v8::Value> value, DataConverter * converter);
}
#line 1 "./src/util/binder.lzz"
class Binder
{
#line 2 "./src/util/binder.lzz"
public:
#line 4 "./src/util/binder.lzz"
  explicit Binder (sqlite3_stmt * _handle);
#line 11 "./src/util/binder.lzz"
  bool Bind (v8::FunctionCallbackInfo <v8 :: Value> const & info, int argc, Statement * stmt);
#line 28 "./src/util/binder.lzz"
private:
#line 30 "./src/util/binder.lzz"
  struct Result
  {
#line 31 "./src/util/binder.lzz"
    int count;
#line 32 "./src/util/binder.lzz"
    bool bound_object;
  };
#line 35 "./src/util/binder.lzz"
  static bool IsPlainObject (v8::Isolate * isolate, v8::Local <v8::Object> obj);
#line 44 "./src/util/binder.lzz"
  void Fail (void (* Throw) (char const *), char const * message);
#line 52 "./src/util/binder.lzz"
  int NextAnonIndex ();
#line 58 "./src/util/binder.lzz"
  void BindValue (v8::Isolate * isolate, v8::Local <v8::Value> value, int index);
#line 79 "./src/util/binder.lzz"
  int BindArray (v8::Isolate * isolate, v8::Local <v8::Array> arr);
#line 105 "./src/util/binder.lzz"
  int BindObject (v8::Isolate * isolate, v8::Local <v8::Object> obj, Statement * stmt);
#line 149 "./src/util/binder.lzz"
  Result BindArgs (v8::FunctionCallbackInfo <v8 :: Value> const & info, int argc, Statement * stmt);
#line 189 "./src/util/binder.lzz"
  sqlite3_stmt * handle;
#line 190 "./src/util/binder.lzz"
  int param_count;
#line 191 "./src/util/binder.lzz"
  int anon_index;
#line 192 "./src/util/binder.lzz"
  bool success;
};
#line 34 "./src/better_sqlite3.lzz"
struct Addon
{
#line 35 "./src/better_sqlite3.lzz"
  static void JS_setErrorConstructor (v8::FunctionCallbackInfo <v8 :: Value> const & info);
#line 40 "./src/better_sqlite3.lzz"
  static void Cleanup (void * ptr);
#line 47 "./src/better_sqlite3.lzz"
  explicit Addon (v8::Isolate * isolate);
#line 52 "./src/better_sqlite3.lzz"
  sqlite3_uint64 NextId ();
#line 56 "./src/better_sqlite3.lzz"
  CopyablePersistent <v8::Function> Statement;
#line 57 "./src/better_sqlite3.lzz"
  CopyablePersistent <v8::Function> StatementIterator;
#line 58 "./src/better_sqlite3.lzz"
  CopyablePersistent <v8::Function> Backup;
#line 59 "./src/better_sqlite3.lzz"
  CopyablePersistent <v8::Function> SqliteError;
#line 60 "./src/better_sqlite3.lzz"
  v8::FunctionCallbackInfo <v8 :: Value> const * privileged_info;
#line 61 "./src/better_sqlite3.lzz"
  sqlite3_uint64 next_id;
#line 62 "./src/better_sqlite3.lzz"
  CS cs;
#line 63 "./src/better_sqlite3.lzz"
  std::set <Database*, Database::CompareDatabase> dbs;
};
#line 16 "./src/util/macros.lzz"
LZZ_INLINE v8::Local <v8::String> StringFromUtf8 (v8::Isolate * isolate, char const * data, int length)
#line 16 "./src/util/macros.lzz"
                                                                                                {
        return v8::String::NewFromUtf8(isolate, data, v8::NewStringType::kNormal, length).ToLocalChecked();
}
#line 19 "./src/util/macros.lzz"
LZZ_INLINE v8::Local <v8::String> InternalizedFromUtf8 (v8::Isolate * isolate, char const * data, int length)
#line 19 "./src/util/macros.lzz"
                                                                                                      {
        return v8::String::NewFromUtf8(isolate, data, v8::NewStringType::kInternalized, length).ToLocalChecked();
}
#line 22 "./src/util/macros.lzz"
LZZ_INLINE v8::Local <v8::Value> InternalizedFromUtf8OrNull (v8::Isolate * isolate, char const * data, int length)
#line 22 "./src/util/macros.lzz"
                                                                                                           {
        if (data == NULL) return v8::Null(isolate);
        return InternalizedFromUtf8(isolate, data, length);
}
#line 26 "./src/util/macros.lzz"
LZZ_INLINE v8::Local <v8::String> InternalizedFromLatin1 (v8::Isolate * isolate, char const * str)
#line 26 "./src/util/macros.lzz"
                                                                                           {
        return v8::String::NewFromOneByte(isolate, reinterpret_cast<const uint8_t*>(str), v8::NewStringType::kInternalized).ToLocalChecked();
}
#line 33 "./src/util/macros.lzz"
LZZ_INLINE void SetFrozen (v8::Isolate * isolate, v8::Local <v8::Context> ctx, v8::Local <v8::Object> obj, CopyablePersistent <v8::String> & key, v8::Local <v8::Value> value)
#line 33 "./src/util/macros.lzz"
                                                                                                                                                                    {
        obj->DefineOwnProperty(ctx, key.Get(isolate), value, static_cast<v8::PropertyAttribute>(v8::DontDelete | v8::ReadOnly)).FromJust();
}
#line 91 "./src/util/macros.lzz"
LZZ_INLINE bool IS_SKIPPED (char c)
#line 91 "./src/util/macros.lzz"
                               {
        return c == ' ' || c == ';' || (c >= '\t' && c <= '\r');
}
#line 96 "./src/util/macros.lzz"
template <typename T>
#line 96 "./src/util/macros.lzz"
LZZ_INLINE T * ALLOC_ARRAY (size_t count)
#line 96 "./src/util/macros.lzz"
                                                      {
        return static_cast<T*>(::operator new[](count * sizeof(T)));
}
#line 101 "./src/util/macros.lzz"
template <typename T>
#line 101 "./src/util/macros.lzz"
LZZ_INLINE void FREE_ARRAY (T * array_pointer)
#line 101 "./src/util/macros.lzz"
                                                           {
        ::operator delete[](array_pointer);
}
#line 9 "./src/util/bind-map.lzz"
LZZ_INLINE int BindMap::Pair::GetIndex ()
#line 9 "./src/util/bind-map.lzz"
                                              {
                                return index;
}
#line 13 "./src/util/bind-map.lzz"
LZZ_INLINE v8::Local <v8::String> BindMap::Pair::GetName (v8::Isolate * isolate)
#line 13 "./src/util/bind-map.lzz"
                                                                                   {
                                return name.Get(isolate);
}
#line 41 "./src/util/bind-map.lzz"
LZZ_INLINE BindMap::Pair * BindMap::GetPairs ()
#line 41 "./src/util/bind-map.lzz"
                                {
                return pairs;
}
#line 45 "./src/util/bind-map.lzz"
LZZ_INLINE int BindMap::GetSize ()
#line 45 "./src/util/bind-map.lzz"
                             {
                return length;
}
#line 77 "./src/objects/database.lzz"
LZZ_INLINE void Database::AddStatement (Statement * stmt)
#line 77 "./src/objects/database.lzz"
                                                  { stmts.insert(stmts.end(), stmt);
}
#line 78 "./src/objects/database.lzz"
LZZ_INLINE void Database::RemoveStatement (Statement * stmt)
#line 78 "./src/objects/database.lzz"
                                                     { stmts.erase(stmt);
}
#line 81 "./src/objects/database.lzz"
LZZ_INLINE void Database::AddBackup (Backup * backup)
#line 81 "./src/objects/database.lzz"
                                              { backups.insert(backups.end(), backup);
}
#line 82 "./src/objects/database.lzz"
LZZ_INLINE void Database::RemoveBackup (Backup * backup)
#line 82 "./src/objects/database.lzz"
                                                 { backups.erase(backup);
}
#line 96 "./src/objects/database.lzz"
LZZ_INLINE Database::State * Database::GetState ()
#line 96 "./src/objects/database.lzz"
                                 {
                return reinterpret_cast<State*>(&open);
}
#line 99 "./src/objects/database.lzz"
LZZ_INLINE sqlite3 * Database::GetHandle ()
#line 99 "./src/objects/database.lzz"
                                    {
                return db_handle;
}
#line 102 "./src/objects/database.lzz"
LZZ_INLINE Addon * Database::GetAddon ()
#line 102 "./src/objects/database.lzz"
                                 {
                return addon;
}
#line 21 "./src/objects/statement.lzz"
LZZ_INLINE bool Statement::Compare (Statement const * const a, Statement const * const b)
#line 21 "./src/objects/statement.lzz"
                                                                                         {
                return a->extras->id < b->extras->id;
}
#line 119 "./src/objects/statement-iterator.lzz"
LZZ_INLINE v8::Local <v8::Object> StatementIterator::NewRecord (v8::Isolate * isolate, v8::Local <v8::Context> ctx, v8::Local <v8::Value> value, Addon * addon, bool done)
#line 119 "./src/objects/statement-iterator.lzz"
                                                                                                                                                             {
                v8::Local<v8::Object> record = v8::Object::New(isolate);
                record->Set(ctx, addon->cs.value.Get(isolate), value).FromJust();
                record->Set(ctx, addon->cs.done.Get(isolate), v8::Boolean::New(isolate, done)).FromJust();
                return record;
}
#line 126 "./src/objects/statement-iterator.lzz"
LZZ_INLINE v8::Local <v8::Object> StatementIterator::DoneRecord (v8::Isolate * isolate, Addon * addon)
#line 126 "./src/objects/statement-iterator.lzz"
                                                                                           {
                return NewRecord(isolate, isolate -> GetCurrentContext ( ) , v8::Undefined(isolate), addon, true);
}
#line 12 "./src/objects/backup.lzz"
LZZ_INLINE bool Backup::Compare (Backup const * const a, Backup const * const b)
#line 12 "./src/objects/backup.lzz"
                                                                                   {
                return a->id < b->id;
}
#line 39 "./src/util/custom-aggregate.lzz"
LZZ_INLINE void CustomAggregate::xStepBase (sqlite3_context * invocation, int argc, sqlite3_value * * argv, CopyablePersistent <v8::Function> const CustomAggregate::* ptrtm)
#line 39 "./src/util/custom-aggregate.lzz"
                                                                                                                                                                  {
                CustomAggregate * self = static_cast < CustomAggregate * > ( sqlite3_user_data ( invocation ) ) ; v8 :: Isolate * isolate = self -> isolate ; v8 :: HandleScope scope ( isolate ) ; Accumulator * acc = self -> GetAccumulator ( invocation ) ; if ( acc -> value . IsEmpty ( ) ) return ;

                v8::Local<v8::Value> args_fast[5];
                v8::Local<v8::Value>* args = argc <= 4 ? args_fast : ALLOC_ARRAY<v8::Local<v8::Value>>(argc + 1);
                args[0] = acc->value.Get(isolate);
                if (argc != 0) Data::GetArgumentsJS(isolate, args + 1, argv, argc, self->safe_ints);

                v8::MaybeLocal<v8::Value> maybeReturnValue = (self->*ptrtm).Get(isolate)->Call( isolate -> GetCurrentContext ( ) , v8::Undefined(isolate), argc + 1, args);
                if (args != args_fast) delete[] args;

                if (maybeReturnValue.IsEmpty()) {
                        self->PropagateJSError(invocation);
                } else {
                        v8::Local<v8::Value> returnValue = maybeReturnValue.ToLocalChecked();
                        if (!returnValue->IsUndefined()) acc->value.Reset(isolate, returnValue);
                }
}
#line 58 "./src/util/custom-aggregate.lzz"
LZZ_INLINE void CustomAggregate::xValueBase (sqlite3_context * invocation, bool is_final)
#line 58 "./src/util/custom-aggregate.lzz"
                                                                                  {
                CustomAggregate * self = static_cast < CustomAggregate * > ( sqlite3_user_data ( invocation ) ) ; v8 :: Isolate * isolate = self -> isolate ; v8 :: HandleScope scope ( isolate ) ; Accumulator * acc = self -> GetAccumulator ( invocation ) ; if ( acc -> value . IsEmpty ( ) ) return ;

                if (!is_final) {
                        acc->is_window = true;
                } else if (acc->is_window) {
                        DestroyAccumulator(invocation);
                        return;
                }

                v8::Local<v8::Value> result = acc->value.Get(isolate);
                if (self->invoke_result) {
                        v8::MaybeLocal<v8::Value> maybeResult = self->result.Get(isolate)->Call( isolate -> GetCurrentContext ( ) , v8::Undefined(isolate), 1, &result);
                        if (maybeResult.IsEmpty()) {
                                self->PropagateJSError(invocation);
                                return;
                        }
                        result = maybeResult.ToLocalChecked();
                }

                Data::ResultValueFromJS(isolate, invocation, result, self);
                if (is_final) DestroyAccumulator(invocation);
}
#line 92 "./src/util/custom-table.lzz"
LZZ_INLINE CustomTable::VTab * CustomTable::VTab::Upcast (sqlite3_vtab * vtab)
#line 92 "./src/util/custom-table.lzz"
                                                                            {
                        return reinterpret_cast<VTab*>(vtab);
}
#line 96 "./src/util/custom-table.lzz"
LZZ_INLINE sqlite3_vtab * CustomTable::VTab::Downcast ()
#line 96 "./src/util/custom-table.lzz"
                                                {
                        return reinterpret_cast<sqlite3_vtab*>(this);
}
#line 110 "./src/util/custom-table.lzz"
LZZ_INLINE CustomTable::Cursor * CustomTable::Cursor::Upcast (sqlite3_vtab_cursor * cursor)
#line 110 "./src/util/custom-table.lzz"
                                                                                       {
                        return reinterpret_cast<Cursor*>(cursor);
}
#line 114 "./src/util/custom-table.lzz"
LZZ_INLINE sqlite3_vtab_cursor * CustomTable::Cursor::Downcast ()
#line 114 "./src/util/custom-table.lzz"
                                                       {
                        return reinterpret_cast<sqlite3_vtab_cursor*>(this);
}
#line 118 "./src/util/custom-table.lzz"
LZZ_INLINE CustomTable::VTab * CustomTable::Cursor::GetVTab ()
#line 118 "./src/util/custom-table.lzz"
                                                    {
                        return VTab::Upcast(base.pVtab);
}
#line 52 "./src/better_sqlite3.lzz"
LZZ_INLINE sqlite3_uint64 Addon::NextId ()
#line 52 "./src/better_sqlite3.lzz"
                                       {
                return next_id++;
}
#undef LZZ_INLINE
#endif
