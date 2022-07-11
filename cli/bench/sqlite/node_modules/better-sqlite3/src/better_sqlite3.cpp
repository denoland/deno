// better_sqlite3.cpp
//

#include "better_sqlite3.hpp"
#line 67 "./src/better_sqlite3.lzz"
NODE_MODULE_INIT(/* exports, context */) {
	v8::Isolate* isolate = context->GetIsolate();
	v8::HandleScope scope(isolate);

	// Initialize addon instance.
	Addon* addon = new Addon(isolate);
	v8::Local<v8::External> data = v8::External::New(isolate, addon);
	node::AddEnvironmentCleanupHook(isolate, Addon::Cleanup, addon);

	// Create and export native-backed classes and functions.
	exports->Set(context, InternalizedFromLatin1(isolate, "Database"), Database::Init(isolate, data)).FromJust();
	exports->Set(context, InternalizedFromLatin1(isolate, "Statement"), Statement::Init(isolate, data)).FromJust();
	exports->Set(context, InternalizedFromLatin1(isolate, "StatementIterator"), StatementIterator::Init(isolate, data)).FromJust();
	exports->Set(context, InternalizedFromLatin1(isolate, "Backup"), Backup::Init(isolate, data)).FromJust();
	exports->Set(context, InternalizedFromLatin1(isolate, "setErrorConstructor"), v8::FunctionTemplate::New(isolate, Addon::JS_setErrorConstructor, data)->GetFunction(context).ToLocalChecked()).FromJust();

	// Store addon instance data.
	addon->Statement.Reset(isolate, exports->Get(context, InternalizedFromLatin1(isolate, "Statement")).ToLocalChecked().As<v8::Function>());
	addon->StatementIterator.Reset(isolate, exports->Get(context, InternalizedFromLatin1(isolate, "StatementIterator")).ToLocalChecked().As<v8::Function>());
	addon->Backup.Reset(isolate, exports->Get(context, InternalizedFromLatin1(isolate, "Backup")).ToLocalChecked().As<v8::Function>());
}
#define LZZ_INLINE inline
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 67 "./src/util/data.lzz"
  static char const FLAT = 0;
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 68 "./src/util/data.lzz"
  static char const PLUCK = 1;
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 69 "./src/util/data.lzz"
  static char const EXPAND = 2;
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 70 "./src/util/data.lzz"
  static char const RAW = 3;
}
#line 37 "./src/util/macros.lzz"
void ThrowError (char const * message)
#line 37 "./src/util/macros.lzz"
                                     { v8 :: Isolate * isolate = v8 :: Isolate :: GetCurrent ( ) ; isolate->ThrowException(v8::Exception::Error(StringFromUtf8(isolate, message, -1)));
}
#line 38 "./src/util/macros.lzz"
void ThrowTypeError (char const * message)
#line 38 "./src/util/macros.lzz"
                                         { v8 :: Isolate * isolate = v8 :: Isolate :: GetCurrent ( ) ; isolate->ThrowException(v8::Exception::TypeError(StringFromUtf8(isolate, message, -1)));
}
#line 39 "./src/util/macros.lzz"
void ThrowRangeError (char const * message)
#line 39 "./src/util/macros.lzz"
                                          { v8 :: Isolate * isolate = v8 :: Isolate :: GetCurrent ( ) ; isolate->ThrowException(v8::Exception::RangeError(StringFromUtf8(isolate, message, -1)));
}
#line 105 "./src/util/macros.lzz"
v8::Local <v8::FunctionTemplate> NewConstructorTemplate (v8::Isolate * isolate, v8::Local <v8::External> data, v8::FunctionCallback func, char const * name)
#line 110 "./src/util/macros.lzz"
  {
        v8::Local<v8::FunctionTemplate> t = v8::FunctionTemplate::New(isolate, func, data);
        t->InstanceTemplate()->SetInternalFieldCount(1);
        t->SetClassName(InternalizedFromLatin1(isolate, name));
        return t;
}
#line 116 "./src/util/macros.lzz"
void SetPrototypeMethod (v8::Isolate * isolate, v8::Local <v8::External> data, v8::Local <v8::FunctionTemplate> recv, char const * name, v8::FunctionCallback func)
#line 122 "./src/util/macros.lzz"
  {
        v8::HandleScope scope(isolate);
        recv->PrototypeTemplate()->Set(
                InternalizedFromLatin1(isolate, name),
                v8::FunctionTemplate::New(isolate, func, data, v8::Signature::New(isolate, recv))
        );
}
#line 129 "./src/util/macros.lzz"
void SetPrototypeSymbolMethod (v8::Isolate * isolate, v8::Local <v8::External> data, v8::Local <v8::FunctionTemplate> recv, v8::Local <v8::Symbol> symbol, v8::FunctionCallback func)
#line 135 "./src/util/macros.lzz"
  {
        v8::HandleScope scope(isolate);
        recv->PrototypeTemplate()->Set(
                symbol,
                v8::FunctionTemplate::New(isolate, func, data, v8::Signature::New(isolate, recv))
        );
}
#line 142 "./src/util/macros.lzz"
void SetPrototypeGetter (v8::Isolate * isolate, v8::Local <v8::External> data, v8::Local <v8::FunctionTemplate> recv, char const * name, v8::AccessorGetterCallback func)
#line 148 "./src/util/macros.lzz"
  {
        v8::HandleScope scope(isolate);
        recv->InstanceTemplate()->SetAccessor(
                InternalizedFromLatin1(isolate, name),
                func,
                0,
                data,
                v8::AccessControl::DEFAULT,
                v8::PropertyAttribute::None,
                v8::AccessorSignature::New(isolate, recv)
        );
}
#line 4 "./src/util/constants.lzz"
v8::Local <v8::String> CS::Code (v8::Isolate * isolate, int code)
#line 4 "./src/util/constants.lzz"
                                                                   {
                auto element = codes.find(code);
                if (element != codes.end()) return element->second.Get(isolate);
                return StringFromUtf8(isolate, (std::string("UNKNOWN_SQLITE_ERROR_") + std::to_string(code)).c_str(), -1);
}
#line 10 "./src/util/constants.lzz"
CS::CS (v8::Isolate * isolate)
#line 10 "./src/util/constants.lzz"
                                          {
                SetString(isolate, database, "database");
                SetString(isolate, reader, "reader");
                SetString(isolate, source, "source");
                SetString(isolate, memory, "memory");
                SetString(isolate, readonly, "readonly");
                SetString(isolate, name, "name");
                SetString(isolate, next, "next");
                SetString(isolate, length, "length");
                SetString(isolate, done, "done");
                SetString(isolate, value, "value");
                SetString(isolate, changes, "changes");
                SetString(isolate, lastInsertRowid, "lastInsertRowid");
                SetString(isolate, statement, "statement");
                SetString(isolate, column, "column");
                SetString(isolate, table, "table");
                SetString(isolate, type, "type");
                SetString(isolate, totalPages, "totalPages");
                SetString(isolate, remainingPages, "remainingPages");

                SetCode(isolate, SQLITE_OK, "SQLITE_OK");
                SetCode(isolate, SQLITE_ERROR, "SQLITE_ERROR");
                SetCode(isolate, SQLITE_INTERNAL, "SQLITE_INTERNAL");
                SetCode(isolate, SQLITE_PERM, "SQLITE_PERM");
                SetCode(isolate, SQLITE_ABORT, "SQLITE_ABORT");
                SetCode(isolate, SQLITE_BUSY, "SQLITE_BUSY");
                SetCode(isolate, SQLITE_LOCKED, "SQLITE_LOCKED");
                SetCode(isolate, SQLITE_NOMEM, "SQLITE_NOMEM");
                SetCode(isolate, SQLITE_READONLY, "SQLITE_READONLY");
                SetCode(isolate, SQLITE_INTERRUPT, "SQLITE_INTERRUPT");
                SetCode(isolate, SQLITE_IOERR, "SQLITE_IOERR");
                SetCode(isolate, SQLITE_CORRUPT, "SQLITE_CORRUPT");
                SetCode(isolate, SQLITE_NOTFOUND, "SQLITE_NOTFOUND");
                SetCode(isolate, SQLITE_FULL, "SQLITE_FULL");
                SetCode(isolate, SQLITE_CANTOPEN, "SQLITE_CANTOPEN");
                SetCode(isolate, SQLITE_PROTOCOL, "SQLITE_PROTOCOL");
                SetCode(isolate, SQLITE_EMPTY, "SQLITE_EMPTY");
                SetCode(isolate, SQLITE_SCHEMA, "SQLITE_SCHEMA");
                SetCode(isolate, SQLITE_TOOBIG, "SQLITE_TOOBIG");
                SetCode(isolate, SQLITE_CONSTRAINT, "SQLITE_CONSTRAINT");
                SetCode(isolate, SQLITE_MISMATCH, "SQLITE_MISMATCH");
                SetCode(isolate, SQLITE_MISUSE, "SQLITE_MISUSE");
                SetCode(isolate, SQLITE_NOLFS, "SQLITE_NOLFS");
                SetCode(isolate, SQLITE_AUTH, "SQLITE_AUTH");
                SetCode(isolate, SQLITE_FORMAT, "SQLITE_FORMAT");
                SetCode(isolate, SQLITE_RANGE, "SQLITE_RANGE");
                SetCode(isolate, SQLITE_NOTADB, "SQLITE_NOTADB");
                SetCode(isolate, SQLITE_NOTICE, "SQLITE_NOTICE");
                SetCode(isolate, SQLITE_WARNING, "SQLITE_WARNING");
                SetCode(isolate, SQLITE_ROW, "SQLITE_ROW");
                SetCode(isolate, SQLITE_DONE, "SQLITE_DONE");
                SetCode(isolate, SQLITE_IOERR_READ, "SQLITE_IOERR_READ");
                SetCode(isolate, SQLITE_IOERR_SHORT_READ, "SQLITE_IOERR_SHORT_READ");
                SetCode(isolate, SQLITE_IOERR_WRITE, "SQLITE_IOERR_WRITE");
                SetCode(isolate, SQLITE_IOERR_FSYNC, "SQLITE_IOERR_FSYNC");
                SetCode(isolate, SQLITE_IOERR_DIR_FSYNC, "SQLITE_IOERR_DIR_FSYNC");
                SetCode(isolate, SQLITE_IOERR_TRUNCATE, "SQLITE_IOERR_TRUNCATE");
                SetCode(isolate, SQLITE_IOERR_FSTAT, "SQLITE_IOERR_FSTAT");
                SetCode(isolate, SQLITE_IOERR_UNLOCK, "SQLITE_IOERR_UNLOCK");
                SetCode(isolate, SQLITE_IOERR_RDLOCK, "SQLITE_IOERR_RDLOCK");
                SetCode(isolate, SQLITE_IOERR_DELETE, "SQLITE_IOERR_DELETE");
                SetCode(isolate, SQLITE_IOERR_BLOCKED, "SQLITE_IOERR_BLOCKED");
                SetCode(isolate, SQLITE_IOERR_NOMEM, "SQLITE_IOERR_NOMEM");
                SetCode(isolate, SQLITE_IOERR_ACCESS, "SQLITE_IOERR_ACCESS");
                SetCode(isolate, SQLITE_IOERR_CHECKRESERVEDLOCK, "SQLITE_IOERR_CHECKRESERVEDLOCK");
                SetCode(isolate, SQLITE_IOERR_LOCK, "SQLITE_IOERR_LOCK");
                SetCode(isolate, SQLITE_IOERR_CLOSE, "SQLITE_IOERR_CLOSE");
                SetCode(isolate, SQLITE_IOERR_DIR_CLOSE, "SQLITE_IOERR_DIR_CLOSE");
                SetCode(isolate, SQLITE_IOERR_SHMOPEN, "SQLITE_IOERR_SHMOPEN");
                SetCode(isolate, SQLITE_IOERR_SHMSIZE, "SQLITE_IOERR_SHMSIZE");
                SetCode(isolate, SQLITE_IOERR_SHMLOCK, "SQLITE_IOERR_SHMLOCK");
                SetCode(isolate, SQLITE_IOERR_SHMMAP, "SQLITE_IOERR_SHMMAP");
                SetCode(isolate, SQLITE_IOERR_SEEK, "SQLITE_IOERR_SEEK");
                SetCode(isolate, SQLITE_IOERR_DELETE_NOENT, "SQLITE_IOERR_DELETE_NOENT");
                SetCode(isolate, SQLITE_IOERR_MMAP, "SQLITE_IOERR_MMAP");
                SetCode(isolate, SQLITE_IOERR_GETTEMPPATH, "SQLITE_IOERR_GETTEMPPATH");
                SetCode(isolate, SQLITE_IOERR_CONVPATH, "SQLITE_IOERR_CONVPATH");
                SetCode(isolate, SQLITE_IOERR_VNODE, "SQLITE_IOERR_VNODE");
                SetCode(isolate, SQLITE_IOERR_AUTH, "SQLITE_IOERR_AUTH");
                SetCode(isolate, SQLITE_LOCKED_SHAREDCACHE, "SQLITE_LOCKED_SHAREDCACHE");
                SetCode(isolate, SQLITE_BUSY_RECOVERY, "SQLITE_BUSY_RECOVERY");
                SetCode(isolate, SQLITE_BUSY_SNAPSHOT, "SQLITE_BUSY_SNAPSHOT");
                SetCode(isolate, SQLITE_CANTOPEN_NOTEMPDIR, "SQLITE_CANTOPEN_NOTEMPDIR");
                SetCode(isolate, SQLITE_CANTOPEN_ISDIR, "SQLITE_CANTOPEN_ISDIR");
                SetCode(isolate, SQLITE_CANTOPEN_FULLPATH, "SQLITE_CANTOPEN_FULLPATH");
                SetCode(isolate, SQLITE_CANTOPEN_CONVPATH, "SQLITE_CANTOPEN_CONVPATH");
                SetCode(isolate, SQLITE_CORRUPT_VTAB, "SQLITE_CORRUPT_VTAB");
                SetCode(isolate, SQLITE_READONLY_RECOVERY, "SQLITE_READONLY_RECOVERY");
                SetCode(isolate, SQLITE_READONLY_CANTLOCK, "SQLITE_READONLY_CANTLOCK");
                SetCode(isolate, SQLITE_READONLY_ROLLBACK, "SQLITE_READONLY_ROLLBACK");
                SetCode(isolate, SQLITE_READONLY_DBMOVED, "SQLITE_READONLY_DBMOVED");
                SetCode(isolate, SQLITE_ABORT_ROLLBACK, "SQLITE_ABORT_ROLLBACK");
                SetCode(isolate, SQLITE_CONSTRAINT_CHECK, "SQLITE_CONSTRAINT_CHECK");
                SetCode(isolate, SQLITE_CONSTRAINT_COMMITHOOK, "SQLITE_CONSTRAINT_COMMITHOOK");
                SetCode(isolate, SQLITE_CONSTRAINT_FOREIGNKEY, "SQLITE_CONSTRAINT_FOREIGNKEY");
                SetCode(isolate, SQLITE_CONSTRAINT_FUNCTION, "SQLITE_CONSTRAINT_FUNCTION");
                SetCode(isolate, SQLITE_CONSTRAINT_NOTNULL, "SQLITE_CONSTRAINT_NOTNULL");
                SetCode(isolate, SQLITE_CONSTRAINT_PRIMARYKEY, "SQLITE_CONSTRAINT_PRIMARYKEY");
                SetCode(isolate, SQLITE_CONSTRAINT_TRIGGER, "SQLITE_CONSTRAINT_TRIGGER");
                SetCode(isolate, SQLITE_CONSTRAINT_UNIQUE, "SQLITE_CONSTRAINT_UNIQUE");
                SetCode(isolate, SQLITE_CONSTRAINT_VTAB, "SQLITE_CONSTRAINT_VTAB");
                SetCode(isolate, SQLITE_CONSTRAINT_ROWID, "SQLITE_CONSTRAINT_ROWID");
                SetCode(isolate, SQLITE_NOTICE_RECOVER_WAL, "SQLITE_NOTICE_RECOVER_WAL");
                SetCode(isolate, SQLITE_NOTICE_RECOVER_ROLLBACK, "SQLITE_NOTICE_RECOVER_ROLLBACK");
                SetCode(isolate, SQLITE_WARNING_AUTOINDEX, "SQLITE_WARNING_AUTOINDEX");
                SetCode(isolate, SQLITE_AUTH_USER, "SQLITE_AUTH_USER");
                SetCode(isolate, SQLITE_OK_LOAD_PERMANENTLY, "SQLITE_OK_LOAD_PERMANENTLY");
}
#line 140 "./src/util/constants.lzz"
void CS::SetString (v8::Isolate * isolate, CopyablePersistent <v8::String> & constant, char const * str)
#line 140 "./src/util/constants.lzz"
                                                                                                               {
                constant.Reset(isolate, InternalizedFromLatin1(isolate, str));
}
#line 144 "./src/util/constants.lzz"
void CS::SetCode (v8::Isolate * isolate, int code, char const * str)
#line 144 "./src/util/constants.lzz"
                                                                      {
                codes.emplace(std::piecewise_construct,
                        std::forward_as_tuple(code),
                        std::forward_as_tuple(isolate, InternalizedFromLatin1(isolate, str)));
}
#line 19 "./src/util/bind-map.lzz"
BindMap::Pair::Pair (v8::Isolate * isolate, char const * name, int index)
#line 20 "./src/util/bind-map.lzz"
  : name (isolate, InternalizedFromUtf8(isolate, name, -1)), index (index)
#line 20 "./src/util/bind-map.lzz"
                                                                                                       {}
#line 22 "./src/util/bind-map.lzz"
BindMap::Pair::Pair (v8::Isolate * isolate, Pair * pair)
#line 23 "./src/util/bind-map.lzz"
  : name (isolate, pair->name), index (pair->index)
#line 23 "./src/util/bind-map.lzz"
                                                                                {}
#line 29 "./src/util/bind-map.lzz"
BindMap::BindMap (char _)
#line 29 "./src/util/bind-map.lzz"
                                 {
                assert(_ == 0);
                pairs = NULL;
                capacity = 0;
                length = 0;
}
#line 36 "./src/util/bind-map.lzz"
BindMap::~ BindMap ()
#line 36 "./src/util/bind-map.lzz"
                   {
                while (length) pairs[--length].~Pair();
                FREE_ARRAY<Pair>(pairs);
}
#line 50 "./src/util/bind-map.lzz"
void BindMap::Add (v8::Isolate * isolate, char const * name, int index)
#line 50 "./src/util/bind-map.lzz"
                                                                    {
                assert(name != NULL);
                if (capacity == length) Grow(isolate);
                new (pairs + length++) Pair(isolate, name, index);
}
#line 58 "./src/util/bind-map.lzz"
void BindMap::Grow (v8::Isolate * isolate)
#line 58 "./src/util/bind-map.lzz"
                                        {
                assert(capacity == length);
                capacity = (capacity << 1) | 2;
                Pair* new_pairs = ALLOC_ARRAY<Pair>(capacity);
                for (int i = 0; i < length; ++i) {
                        new (new_pairs + i) Pair(isolate, pairs + i);
                        pairs[i].~Pair();
                }
                FREE_ARRAY<Pair>(pairs);
                pairs = new_pairs;
}
#line 4 "./src/objects/database.lzz"
v8::Local <v8 :: Function> Database::Init (v8::Isolate * isolate, v8::Local <v8 :: External> data)
#line 4 "./src/objects/database.lzz"
                   {
                v8::Local<v8::FunctionTemplate> t = NewConstructorTemplate(isolate, data, JS_new, "Database");
                SetPrototypeMethod(isolate, data, t, "prepare", JS_prepare);
                SetPrototypeMethod(isolate, data, t, "exec", JS_exec);
                SetPrototypeMethod(isolate, data, t, "backup", JS_backup);
                SetPrototypeMethod(isolate, data, t, "serialize", JS_serialize);
                SetPrototypeMethod(isolate, data, t, "function", JS_function);
                SetPrototypeMethod(isolate, data, t, "aggregate", JS_aggregate);
                SetPrototypeMethod(isolate, data, t, "table", JS_table);
                SetPrototypeMethod(isolate, data, t, "loadExtension", JS_loadExtension);
                SetPrototypeMethod(isolate, data, t, "close", JS_close);
                SetPrototypeMethod(isolate, data, t, "defaultSafeIntegers", JS_defaultSafeIntegers);
                SetPrototypeMethod(isolate, data, t, "unsafeMode", JS_unsafeMode);
                SetPrototypeGetter(isolate, data, t, "open", JS_open);
                SetPrototypeGetter(isolate, data, t, "inTransaction", JS_inTransaction);
                return t->GetFunction( isolate -> GetCurrentContext ( ) ).ToLocalChecked();
}
#line 24 "./src/objects/database.lzz"
bool Database::CompareDatabase::operator () (Database const * const a, Database const * const b) const
#line 24 "./src/objects/database.lzz"
                                                                                           {
                        return a < b;
}
#line 29 "./src/objects/database.lzz"
bool Database::CompareStatement::operator () (Statement const * const a, Statement const * const b) const
#line 29 "./src/objects/database.lzz"
                                                                                             {
                        return Statement::Compare(a, b);
}
#line 34 "./src/objects/database.lzz"
bool Database::CompareBackup::operator () (Backup const * const a, Backup const * const b) const
#line 34 "./src/objects/database.lzz"
                                                                                       {
                        return Backup::Compare(a, b);
}
#line 40 "./src/objects/database.lzz"
void Database::ThrowDatabaseError ()
#line 40 "./src/objects/database.lzz"
                                  {
                if (was_js_error) was_js_error = false;
                else ThrowSqliteError(addon, db_handle);
}
#line 44 "./src/objects/database.lzz"
void Database::ThrowSqliteError (Addon * addon, sqlite3 * db_handle)
#line 44 "./src/objects/database.lzz"
                                                                       {
                assert(db_handle != NULL);
                ThrowSqliteError(addon, sqlite3_errmsg(db_handle), sqlite3_extended_errcode(db_handle));
}
#line 48 "./src/objects/database.lzz"
void Database::ThrowSqliteError (Addon * addon, char const * message, int code)
#line 48 "./src/objects/database.lzz"
                                                                                  {
                assert(message != NULL);
                assert((code & 0xff) != SQLITE_OK);
                assert((code & 0xff) != SQLITE_ROW);
                assert((code & 0xff) != SQLITE_DONE);
                v8 :: Isolate * isolate = v8 :: Isolate :: GetCurrent ( ) ;
                v8::Local<v8::Value> args[2] = {
                        StringFromUtf8(isolate, message, -1),
                        addon->cs.Code(isolate, code)
                };
                isolate->ThrowException(addon->SqliteError.Get(isolate)
                        ->NewInstance( isolate -> GetCurrentContext ( ) , 2, args)
                        .ToLocalChecked());
}
#line 64 "./src/objects/database.lzz"
bool Database::Log (v8::Isolate * isolate, sqlite3_stmt * handle)
#line 64 "./src/objects/database.lzz"
                                                             {
                assert(was_js_error == false);
                if (!has_logger) return false;
                char* expanded = sqlite3_expanded_sql(handle);
                v8::Local<v8::Value> arg = StringFromUtf8(isolate, expanded ? expanded : sqlite3_sql(handle), -1);
                was_js_error = logger.Get(isolate).As<v8::Function>()
                        ->Call( isolate -> GetCurrentContext ( ) , v8::Undefined(isolate), 1, &arg)
                        .IsEmpty();
                if (expanded) sqlite3_free(expanded);
                return was_js_error;
}
#line 107 "./src/objects/database.lzz"
void Database::CloseHandles ()
#line 107 "./src/objects/database.lzz"
                            {
                if (open) {
                        open = false;
                        for (Statement* stmt : stmts) stmt->CloseHandles();
                        for (Backup* backup : backups) backup->CloseHandles();
                        stmts.clear();
                        backups.clear();
                        int status = sqlite3_close(db_handle);
                        assert(status == SQLITE_OK); ((void)status);
                }
}
#line 119 "./src/objects/database.lzz"
Database::~ Database ()
#line 119 "./src/objects/database.lzz"
                    {
                if (open) addon->dbs.erase(this);
                CloseHandles();
}
#line 126 "./src/objects/database.lzz"
Database::Database (v8::Isolate * isolate, Addon * addon, sqlite3 * db_handle, v8::Local <v8::Value> logger)
#line 131 "./src/objects/database.lzz"
  : node::ObjectWrap (), db_handle (db_handle), open (true), busy (false), safe_ints (false), unsafe_mode (false), was_js_error (false), has_logger (logger->IsFunction()), iterators (0), addon (addon), logger (isolate, logger), stmts (), backups ()
#line 144 "./src/objects/database.lzz"
                          {
                assert(db_handle != NULL);
                addon->dbs.insert(this);
}
#line 149 "./src/objects/database.lzz"
void Database::JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 149 "./src/objects/database.lzz"
                            {
                assert(info.IsConstructCall());
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > filename = ( info [ 0 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > filenameGiven = ( info [ 1 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 2 ) || ! info [ 2 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "third" " argument to be " "a boolean" ) ; bool in_memory = ( info [ 2 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 3 ) || ! info [ 3 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "fourth" " argument to be " "a boolean" ) ; bool readonly = ( info [ 3 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 4 ) || ! info [ 4 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "fifth" " argument to be " "a boolean" ) ; bool must_exist = ( info [ 4 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 5 ) || ! info [ 5 ] -> IsInt32 ( ) ) return ThrowTypeError ( "Expected " "sixth" " argument to be " "a 32-bit signed integer" ) ; int timeout = ( info [ 5 ] . As < v8 :: Int32 > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 6 ) ) return ThrowTypeError ( "Expected a " "seventh" " argument" ) ; v8 :: Local < v8 :: Value > logger = info [ 6 ] ;
                if ( info . Length ( ) <= ( 7 ) ) return ThrowTypeError ( "Expected a " "eighth" " argument" ) ; v8 :: Local < v8 :: Value > buffer = info [ 7 ] ;

                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                sqlite3* db_handle;
                v8::String::Utf8Value utf8(isolate, filename);
                int mask = readonly ? SQLITE_OPEN_READONLY
                        : must_exist ? SQLITE_OPEN_READWRITE
                        : (SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE);

                if (sqlite3_open_v2(*utf8, &db_handle, mask, NULL) != SQLITE_OK) {
                        ThrowSqliteError(addon, db_handle);
                        int status = sqlite3_close(db_handle);
                        assert(status == SQLITE_OK); ((void)status);
                        return;
                }

                assert(sqlite3_db_mutex(db_handle) == NULL);
                sqlite3_extended_result_codes(db_handle, 1);
                sqlite3_busy_timeout(db_handle, timeout);
                sqlite3_limit(db_handle, SQLITE_LIMIT_LENGTH, MAX_BUFFER_SIZE < MAX_STRING_SIZE ? MAX_BUFFER_SIZE : MAX_STRING_SIZE);
                sqlite3_limit(db_handle, SQLITE_LIMIT_SQL_LENGTH, MAX_STRING_SIZE);
                int status = sqlite3_db_config(db_handle, SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION, 1, NULL);
                assert(status == SQLITE_OK);
                status = sqlite3_db_config(db_handle, SQLITE_DBCONFIG_DEFENSIVE, 1, NULL);
                assert(status == SQLITE_OK);

                if (node::Buffer::HasInstance(buffer) && !Deserialize(buffer.As<v8::Object>(), addon, db_handle, readonly)) {
                        int status = sqlite3_close(db_handle);
                        assert(status == SQLITE_OK); ((void)status);
                        return;
                }

                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                Database* db = new Database(isolate, addon, db_handle, logger);
                db->Wrap(info.This());
                SetFrozen(isolate, ctx, info.This(), addon->cs.memory, v8::Boolean::New(isolate, in_memory));
                SetFrozen(isolate, ctx, info.This(), addon->cs.readonly, v8::Boolean::New(isolate, readonly));
                SetFrozen(isolate, ctx, info.This(), addon->cs.name, filenameGiven);

                info.GetReturnValue().Set(info.This());
}
#line 201 "./src/objects/database.lzz"
void Database::JS_prepare (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 201 "./src/objects/database.lzz"
                                {
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > source = ( info [ 0 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsObject ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "an object" ) ; v8 :: Local < v8 :: Object > database = ( info [ 1 ] . As < v8 :: Object > ( ) ) ;
                if ( info . Length ( ) <= ( 2 ) || ! info [ 2 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "third" " argument to be " "a boolean" ) ; bool pragmaMode = ( info [ 2 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                (void)source;
                (void)database;
                (void)pragmaMode;
                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::Local<v8::Function> c = addon->Statement.Get(isolate);
                addon->privileged_info = &info;
                v8::MaybeLocal<v8::Object> maybeStatement = c->NewInstance( isolate -> GetCurrentContext ( ) , 0, NULL);
                addon->privileged_info = NULL;
                if (!maybeStatement.IsEmpty()) info.GetReturnValue().Set(maybeStatement.ToLocalChecked());
}
#line 217 "./src/objects/database.lzz"
void Database::JS_exec (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 217 "./src/objects/database.lzz"
                             {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > source = ( info [ 0 ] . As < v8 :: String > ( ) ) ;
                if ( ! db -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( ! db -> unsafe_mode ) { if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; } ( ( void ) 0 ) ;
                db->busy = true;

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::String::Utf8Value utf8(isolate, source);
                const char* sql = *utf8;
                const char* tail;

                int status;
                const bool has_logger = db->has_logger;
                sqlite3* const db_handle = db->db_handle;
                sqlite3_stmt* handle;

                for (;;) {
                        while (IS_SKIPPED(*sql)) ++sql;
                        status = sqlite3_prepare_v2(db_handle, sql, -1, &handle, &tail);
                        sql = tail;
                        if (!handle) break;
                        if (has_logger && db->Log(isolate, handle)) {
                                sqlite3_finalize(handle);
                                status = -1;
                                break;
                        }
                        do status = sqlite3_step(handle);
                        while (status == SQLITE_ROW);
                        status = sqlite3_finalize(handle);
                        if (status != SQLITE_OK) break;
                }

                db->busy = false;
                if (status != SQLITE_OK) {
                        db->ThrowDatabaseError();
                }
}
#line 257 "./src/objects/database.lzz"
void Database::JS_backup (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 257 "./src/objects/database.lzz"
                               {
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsObject ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "an object" ) ; v8 :: Local < v8 :: Object > database = ( info [ 0 ] . As < v8 :: Object > ( ) ) ;
                if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > attachedName = ( info [ 1 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 2 ) || ! info [ 2 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "third" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > destFile = ( info [ 2 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 3 ) || ! info [ 3 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "fourth" " argument to be " "a boolean" ) ; bool unlink = ( info [ 3 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                (void)database;
                (void)attachedName;
                (void)destFile;
                (void)unlink;
                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::Local<v8::Function> c = addon->Backup.Get(isolate);
                addon->privileged_info = &info;
                v8::MaybeLocal<v8::Object> maybeBackup = c->NewInstance( isolate -> GetCurrentContext ( ) , 0, NULL);
                addon->privileged_info = NULL;
                if (!maybeBackup.IsEmpty()) info.GetReturnValue().Set(maybeBackup.ToLocalChecked());
}
#line 275 "./src/objects/database.lzz"
void Database::JS_serialize (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 275 "./src/objects/database.lzz"
                                  {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > attachedName = ( info [ 0 ] . As < v8 :: String > ( ) ) ;
                if ( ! db -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::String::Utf8Value attached_name(isolate, attachedName);
                sqlite3_int64 length = -1;
                unsigned char* data = sqlite3_serialize(db->db_handle, *attached_name, &length, 0);

                if (!data && length) {
                        ThrowError("Out of memory");
                        return;
                }

                info.GetReturnValue().Set(
                        node::Buffer::New(isolate, reinterpret_cast<char*>(data), length, FreeSerialization, NULL).ToLocalChecked()
                );
}
#line 297 "./src/objects/database.lzz"
void Database::JS_function (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 297 "./src/objects/database.lzz"
                                 {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsFunction ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a function" ) ; v8 :: Local < v8 :: Function > fn = ( info [ 0 ] . As < v8 :: Function > ( ) ) ;
                if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > nameString = ( info [ 1 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 2 ) || ! info [ 2 ] -> IsInt32 ( ) ) return ThrowTypeError ( "Expected " "third" " argument to be " "a 32-bit signed integer" ) ; int argc = ( info [ 2 ] . As < v8 :: Int32 > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 3 ) || ! info [ 3 ] -> IsInt32 ( ) ) return ThrowTypeError ( "Expected " "fourth" " argument to be " "a 32-bit signed integer" ) ; int safe_ints = ( info [ 3 ] . As < v8 :: Int32 > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 4 ) || ! info [ 4 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "fifth" " argument to be " "a boolean" ) ; bool deterministic = ( info [ 4 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 5 ) || ! info [ 5 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "sixth" " argument to be " "a boolean" ) ; bool direct_only = ( info [ 5 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( ! db -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::String::Utf8Value name(isolate, nameString);
                int mask = SQLITE_UTF8;
                if (deterministic) mask |= SQLITE_DETERMINISTIC;
                if (direct_only) mask |= SQLITE_DIRECTONLY;
                safe_ints = safe_ints < 2 ? safe_ints : static_cast<int>(db->safe_ints);

                if (sqlite3_create_function_v2(db->db_handle, *name, argc, mask, new CustomFunction(isolate, db, *name, fn, safe_ints), CustomFunction::xFunc, NULL, NULL, CustomFunction::xDestroy) != SQLITE_OK) {
                        db->ThrowDatabaseError();
                }
}
#line 321 "./src/objects/database.lzz"
void Database::JS_aggregate (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 321 "./src/objects/database.lzz"
                                  {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if ( info . Length ( ) <= ( 0 ) ) return ThrowTypeError ( "Expected a " "first" " argument" ) ; v8 :: Local < v8 :: Value > start = info [ 0 ] ;
                if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsFunction ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "a function" ) ; v8 :: Local < v8 :: Function > step = ( info [ 1 ] . As < v8 :: Function > ( ) ) ;
                if ( info . Length ( ) <= ( 2 ) ) return ThrowTypeError ( "Expected a " "third" " argument" ) ; v8 :: Local < v8 :: Value > inverse = info [ 2 ] ;
                if ( info . Length ( ) <= ( 3 ) ) return ThrowTypeError ( "Expected a " "fourth" " argument" ) ; v8 :: Local < v8 :: Value > result = info [ 3 ] ;
                if ( info . Length ( ) <= ( 4 ) || ! info [ 4 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "fifth" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > nameString = ( info [ 4 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 5 ) || ! info [ 5 ] -> IsInt32 ( ) ) return ThrowTypeError ( "Expected " "sixth" " argument to be " "a 32-bit signed integer" ) ; int argc = ( info [ 5 ] . As < v8 :: Int32 > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 6 ) || ! info [ 6 ] -> IsInt32 ( ) ) return ThrowTypeError ( "Expected " "seventh" " argument to be " "a 32-bit signed integer" ) ; int safe_ints = ( info [ 6 ] . As < v8 :: Int32 > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 7 ) || ! info [ 7 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "eighth" " argument to be " "a boolean" ) ; bool deterministic = ( info [ 7 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( info . Length ( ) <= ( 8 ) || ! info [ 8 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "ninth" " argument to be " "a boolean" ) ; bool direct_only = ( info [ 8 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( ! db -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::String::Utf8Value name(isolate, nameString);
                auto xInverse = inverse->IsFunction() ? CustomAggregate::xInverse : NULL;
                auto xValue = xInverse ? CustomAggregate::xValue : NULL;
                int mask = SQLITE_UTF8;
                if (deterministic) mask |= SQLITE_DETERMINISTIC;
                if (direct_only) mask |= SQLITE_DIRECTONLY;
                safe_ints = safe_ints < 2 ? safe_ints : static_cast<int>(db->safe_ints);

                if (sqlite3_create_window_function(db->db_handle, *name, argc, mask, new CustomAggregate(isolate, db, *name, start, step, inverse, result, safe_ints), CustomAggregate::xStep, CustomAggregate::xFinal, xValue, xInverse, CustomAggregate::xDestroy) != SQLITE_OK) {
                        db->ThrowDatabaseError();
                }
}
#line 350 "./src/objects/database.lzz"
void Database::JS_table (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 350 "./src/objects/database.lzz"
                              {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsFunction ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a function" ) ; v8 :: Local < v8 :: Function > factory = ( info [ 0 ] . As < v8 :: Function > ( ) ) ;
                if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > nameString = ( info [ 1 ] . As < v8 :: String > ( ) ) ;
                if ( info . Length ( ) <= ( 2 ) || ! info [ 2 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "third" " argument to be " "a boolean" ) ; bool eponymous = ( info [ 2 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ;
                if ( ! db -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::String::Utf8Value name(isolate, nameString);
                sqlite3_module* module = eponymous ? &CustomTable::EPONYMOUS_MODULE : &CustomTable::MODULE;

                db->busy = true;
                if (sqlite3_create_module_v2(db->db_handle, *name, module, new CustomTable(isolate, db, *name, factory), CustomTable::Destructor) != SQLITE_OK) {
                        db->ThrowDatabaseError();
                }
                db->busy = false;
}
#line 370 "./src/objects/database.lzz"
void Database::JS_loadExtension (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 370 "./src/objects/database.lzz"
                                      {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                v8::Local<v8::String> entryPoint;
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a string" ) ; v8 :: Local < v8 :: String > filename = ( info [ 0 ] . As < v8 :: String > ( ) ) ;
                if (info.Length() > 1) { if ( info . Length ( ) <= ( 1 ) || ! info [ 1 ] -> IsString ( ) ) return ThrowTypeError ( "Expected " "second" " argument to be " "a string" ) ; entryPoint = ( info [ 1 ] . As < v8 :: String > ( ) ) ; }
                if ( ! db -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                char* error;
                int status = sqlite3_load_extension(
                        db->db_handle,
                        *v8::String::Utf8Value(isolate, filename),
                        entryPoint.IsEmpty() ? NULL : *v8::String::Utf8Value(isolate, entryPoint),
                        &error
                );
                if (status != SQLITE_OK) {
                        ThrowSqliteError(db->addon, error, status);
                }
                sqlite3_free(error);
}
#line 392 "./src/objects/database.lzz"
void Database::JS_close (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 392 "./src/objects/database.lzz"
                              {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if (db->open) {
                        if ( db -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                        if ( db -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                        db->addon->dbs.erase(db);
                        db->CloseHandles();
                }
}
#line 402 "./src/objects/database.lzz"
void Database::JS_defaultSafeIntegers (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 402 "./src/objects/database.lzz"
                                            {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if (info.Length() == 0) db->safe_ints = true;
                else { if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a boolean" ) ; db -> safe_ints = ( info [ 0 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ; }
}
#line 408 "./src/objects/database.lzz"
void Database::JS_unsafeMode (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 408 "./src/objects/database.lzz"
                                   {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                if (info.Length() == 0) db->unsafe_mode = true;
                else { if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a boolean" ) ; db -> unsafe_mode = ( info [ 0 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ; }
                sqlite3_db_config(db->db_handle, SQLITE_DBCONFIG_DEFENSIVE, static_cast<int>(!db->unsafe_mode), NULL);
}
#line 415 "./src/objects/database.lzz"
void Database::JS_open (v8::Local <v8 :: String> _, v8::PropertyCallbackInfo <v8 :: Value> const & info)
#line 415 "./src/objects/database.lzz"
                             {
                info.GetReturnValue().Set( node :: ObjectWrap :: Unwrap <Database>(info.This())->open);
}
#line 419 "./src/objects/database.lzz"
void Database::JS_inTransaction (v8::Local <v8 :: String> _, v8::PropertyCallbackInfo <v8 :: Value> const & info)
#line 419 "./src/objects/database.lzz"
                                      {
                Database* db = node :: ObjectWrap :: Unwrap <Database>(info.This());
                info.GetReturnValue().Set(db->open && !static_cast<bool>(sqlite3_get_autocommit(db->db_handle)));
}
#line 424 "./src/objects/database.lzz"
bool Database::Deserialize (v8::Local <v8::Object> buffer, Addon * addon, sqlite3 * db_handle, bool readonly)
#line 424 "./src/objects/database.lzz"
                                                                                                               {
                size_t length = node::Buffer::Length(buffer);
                unsigned char* data = (unsigned char*)sqlite3_malloc64(length);
                unsigned int flags = SQLITE_DESERIALIZE_FREEONCLOSE | SQLITE_DESERIALIZE_RESIZEABLE;

                if (readonly) {
                        flags |= SQLITE_DESERIALIZE_READONLY;
                }
                if (length) {
                        if (!data) {
                                ThrowError("Out of memory");
                                return false;
                        }
                        memcpy(data, node::Buffer::Data(buffer), length);
                }

                int status = sqlite3_deserialize(db_handle, "main", data, length, length, flags);
                if (status != SQLITE_OK) {
                        ThrowSqliteError(addon, status == SQLITE_ERROR ? "unable to deserialize database" : sqlite3_errstr(status), status);
                        return false;
                }

                return true;
}
#line 449 "./src/objects/database.lzz"
void Database::FreeSerialization (char * data, void * _)
#line 449 "./src/objects/database.lzz"
                                                           {
                sqlite3_free(data);
}
#line 453 "./src/objects/database.lzz"
int const Database::MAX_BUFFER_SIZE;
#line 454 "./src/objects/database.lzz"
int const Database::MAX_STRING_SIZE;
#line 4 "./src/objects/statement.lzz"
v8::Local <v8 :: Function> Statement::Init (v8::Isolate * isolate, v8::Local <v8 :: External> data)
#line 4 "./src/objects/statement.lzz"
                   {
                v8::Local<v8::FunctionTemplate> t = NewConstructorTemplate(isolate, data, JS_new, "Statement");
                SetPrototypeMethod(isolate, data, t, "run", JS_run);
                SetPrototypeMethod(isolate, data, t, "get", JS_get);
                SetPrototypeMethod(isolate, data, t, "all", JS_all);
                SetPrototypeMethod(isolate, data, t, "iterate", JS_iterate);
                SetPrototypeMethod(isolate, data, t, "bind", JS_bind);
                SetPrototypeMethod(isolate, data, t, "pluck", JS_pluck);
                SetPrototypeMethod(isolate, data, t, "expand", JS_expand);
                SetPrototypeMethod(isolate, data, t, "raw", JS_raw);
                SetPrototypeMethod(isolate, data, t, "safeIntegers", JS_safeIntegers);
                SetPrototypeMethod(isolate, data, t, "columns", JS_columns);
                SetPrototypeGetter(isolate, data, t, "busy", JS_busy);
                return t->GetFunction( isolate -> GetCurrentContext ( ) ).ToLocalChecked();
}
#line 26 "./src/objects/statement.lzz"
BindMap * Statement::GetBindMap (v8::Isolate * isolate)
#line 26 "./src/objects/statement.lzz"
                                                  {
                if (has_bind_map) return &extras->bind_map;
                BindMap* bind_map = &extras->bind_map;
                int param_count = sqlite3_bind_parameter_count(handle);
                for (int i = 1; i <= param_count; ++i) {
                        const char* name = sqlite3_bind_parameter_name(handle, i);
                        if (name != NULL) bind_map->Add(isolate, name + 1, i);
                }
                has_bind_map = true;
                return bind_map;
}
#line 39 "./src/objects/statement.lzz"
void Statement::CloseHandles ()
#line 39 "./src/objects/statement.lzz"
                            {
                if (alive) {
                        alive = false;
                        sqlite3_finalize(handle);
                }
}
#line 46 "./src/objects/statement.lzz"
Statement::~ Statement ()
#line 46 "./src/objects/statement.lzz"
                     {
                if (alive) db->RemoveStatement(this);
                CloseHandles();
                delete extras;
}
#line 56 "./src/objects/statement.lzz"
Statement::Extras::Extras (sqlite3_uint64 id)
#line 56 "./src/objects/statement.lzz"
  : bind_map (0), id (id)
#line 56 "./src/objects/statement.lzz"
                                                                         {}
#line 61 "./src/objects/statement.lzz"
Statement::Statement (Database * db, sqlite3_stmt * handle, sqlite3_uint64 id, bool returns_data)
#line 66 "./src/objects/statement.lzz"
  : node::ObjectWrap (), db (db), handle (handle), extras (new Extras(id)), alive (true), locked (false), bound (false), has_bind_map (false), safe_ints (db->GetState()->safe_ints), mode (Data::FLAT), returns_data (returns_data)
#line 77 "./src/objects/statement.lzz"
                                           {
                assert(db != NULL);
                assert(handle != NULL);
                assert(db->GetState()->open);
                assert(!db->GetState()->busy);
                db->AddStatement(this);
}
#line 85 "./src/objects/statement.lzz"
void Statement::JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 85 "./src/objects/statement.lzz"
                            {
                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                if (!addon->privileged_info) {
                        return ThrowTypeError("Statements can only be constructed by the db.prepare() method");
                }
                assert(info.IsConstructCall());
                Database* db = node :: ObjectWrap :: Unwrap <Database>(addon->privileged_info->This());
                if ( ! db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;

                v8::Local<v8::String> source = (*addon->privileged_info)[0].As<v8::String>();
                v8::Local<v8::Object> database = (*addon->privileged_info)[1].As<v8::Object>();
                bool pragmaMode = (*addon->privileged_info)[2].As<v8::Boolean>()->Value();
                int flags = SQLITE_PREPARE_PERSISTENT;

                if (pragmaMode) {
                        if ( ! db -> GetState ( ) -> unsafe_mode ) { if ( db -> GetState ( ) -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; } ( ( void ) 0 ) ;
                        flags = 0;
                }

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::String::Utf8Value utf8(isolate, source);
                sqlite3_stmt* handle;
                const char* tail;

                if (sqlite3_prepare_v3(db->GetHandle(), *utf8, utf8.length() + 1, flags, &handle, &tail) != SQLITE_OK) {
                        return db->ThrowDatabaseError();
                }
                if (handle == NULL) {
                        return ThrowRangeError("The supplied SQL string contains no statements");
                }
                for (char c; (c = *tail); ++tail) {
                        if (IS_SKIPPED(c)) continue;
                        if (c == '/' && tail[1] == '*') {
                                tail += 2;
                                for (char c; (c = *tail); ++tail) {
                                        if (c == '*' && tail[1] == '/') {
                                                tail += 1;
                                                break;
                                        }
                                }
                        } else if (c == '-' && tail[1] == '-') {
                                tail += 2;
                                for (char c; (c = *tail); ++tail) {
                                        if (c == '\n') break;
                                }
                        } else {
                                sqlite3_finalize(handle);
                                return ThrowRangeError("The supplied SQL string contains more than one statement");
                        }
                }

                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                bool returns_data = sqlite3_column_count(handle) >= 1 || pragmaMode;
                Statement* stmt = new Statement(db, handle, addon->NextId(), returns_data);
                stmt->Wrap(info.This());
                SetFrozen(isolate, ctx, info.This(), addon->cs.reader, v8::Boolean::New(isolate, returns_data));
                SetFrozen(isolate, ctx, info.This(), addon->cs.readonly, v8::Boolean::New(isolate, sqlite3_stmt_readonly(handle) != 0));
                SetFrozen(isolate, ctx, info.This(), addon->cs.source, source);
                SetFrozen(isolate, ctx, info.This(), addon->cs.database, database);

                info.GetReturnValue().Set(info.This());
}
#line 149 "./src/objects/statement.lzz"
void Statement::JS_run (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 149 "./src/objects/statement.lzz"
                            {
                Statement * stmt = node :: ObjectWrap :: Unwrap < Statement > ( info . This ( ) ) ; ( ( void ) 0 ) ; sqlite3_stmt * handle = stmt -> handle ; Database * db = stmt -> db ; if ( ! db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ; if ( db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ; if ( ! db -> GetState ( ) -> unsafe_mode ) { if ( db -> GetState ( ) -> iterators ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; } ( ( void ) 0 ) ; const bool bound = stmt -> bound ; if ( ! bound ) { Binder binder ( handle ) ; if ( ! binder . Bind ( info , info . Length ( ) , stmt ) ) { sqlite3_clear_bindings ( handle ) ; return ; } ( ( void ) 0 ) ; } else if ( info . Length ( ) > 0 ) { return ThrowTypeError ( "This statement already has bound parameters" ) ; } ( ( void ) 0 ) ; db -> GetState ( ) -> busy = true ; v8 :: Isolate * isolate = info . GetIsolate ( ) ; if ( db -> Log ( isolate , handle ) ) { db -> GetState ( ) -> busy = false ; db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ; } ( ( void ) 0 ) ;
                sqlite3* db_handle = db->GetHandle();
                int total_changes_before = sqlite3_total_changes(db_handle);

                sqlite3_step(handle);
                if (sqlite3_reset(handle) == SQLITE_OK) {
                        int changes = sqlite3_total_changes(db_handle) == total_changes_before ? 0 : sqlite3_changes(db_handle);
                        sqlite3_int64 id = sqlite3_last_insert_rowid(db_handle);
                        Addon* addon = db->GetAddon();
                        v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                        v8::Local<v8::Object> result = v8::Object::New(isolate);
                        result->Set(ctx, addon->cs.changes.Get(isolate), v8::Int32::New(isolate, changes)).FromJust();
                        result->Set(ctx, addon->cs.lastInsertRowid.Get(isolate),
                                stmt->safe_ints
                                        ? v8::BigInt::New(isolate, id).As<v8::Value>()
                                        : v8::Number::New(isolate, (double)id).As<v8::Value>()
                        ).FromJust();
                        db -> GetState ( ) -> busy = false ; info . GetReturnValue ( ) . Set ( result ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
                }
                db -> GetState ( ) -> busy = false ; db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
}
#line 172 "./src/objects/statement.lzz"
void Statement::JS_get (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 172 "./src/objects/statement.lzz"
                            {
                Statement * stmt = node :: ObjectWrap :: Unwrap < Statement > ( info . This ( ) ) ; if ( ! stmt -> returns_data ) return ThrowTypeError ( "This statement does not return data. Use run() instead" ) ; sqlite3_stmt * handle = stmt -> handle ; Database * db = stmt -> db ; if ( ! db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ; if ( db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ; const bool bound = stmt -> bound ; if ( ! bound ) { Binder binder ( handle ) ; if ( ! binder . Bind ( info , info . Length ( ) , stmt ) ) { sqlite3_clear_bindings ( handle ) ; return ; } ( ( void ) 0 ) ; } else if ( info . Length ( ) > 0 ) { return ThrowTypeError ( "This statement already has bound parameters" ) ; } ( ( void ) 0 ) ; db -> GetState ( ) -> busy = true ; v8 :: Isolate * isolate = info . GetIsolate ( ) ; if ( db -> Log ( isolate , handle ) ) { db -> GetState ( ) -> busy = false ; db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ; } ( ( void ) 0 ) ;
                int status = sqlite3_step(handle);
                if (status == SQLITE_ROW) {
                        v8::Local<v8::Value> result = Data::GetRowJS(isolate, isolate -> GetCurrentContext ( ) , handle, stmt->safe_ints, stmt->mode);
                        sqlite3_reset(handle);
                        db -> GetState ( ) -> busy = false ; info . GetReturnValue ( ) . Set ( result ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
                } else if (status == SQLITE_DONE) {
                        sqlite3_reset(handle);
                        db -> GetState ( ) -> busy = false ; info . GetReturnValue ( ) . Set ( v8 :: Undefined ( isolate ) ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
                }
                sqlite3_reset(handle);
                db -> GetState ( ) -> busy = false ; db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
}
#line 187 "./src/objects/statement.lzz"
void Statement::JS_all (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 187 "./src/objects/statement.lzz"
                            {
                Statement * stmt = node :: ObjectWrap :: Unwrap < Statement > ( info . This ( ) ) ; if ( ! stmt -> returns_data ) return ThrowTypeError ( "This statement does not return data. Use run() instead" ) ; sqlite3_stmt * handle = stmt -> handle ; Database * db = stmt -> db ; if ( ! db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ; if ( db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ; const bool bound = stmt -> bound ; if ( ! bound ) { Binder binder ( handle ) ; if ( ! binder . Bind ( info , info . Length ( ) , stmt ) ) { sqlite3_clear_bindings ( handle ) ; return ; } ( ( void ) 0 ) ; } else if ( info . Length ( ) > 0 ) { return ThrowTypeError ( "This statement already has bound parameters" ) ; } ( ( void ) 0 ) ; db -> GetState ( ) -> busy = true ; v8 :: Isolate * isolate = info . GetIsolate ( ) ; if ( db -> Log ( isolate , handle ) ) { db -> GetState ( ) -> busy = false ; db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ; } ( ( void ) 0 ) ;
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                v8::Local<v8::Array> result = v8::Array::New(isolate, 0);
                uint32_t row_count = 0;
                const bool safe_ints = stmt->safe_ints;
                const char mode = stmt->mode;
                bool js_error = false;

                while (sqlite3_step(handle) == SQLITE_ROW) {
                        if (row_count == 0xffffffff) { ThrowRangeError("Array overflow (too many rows returned)"); js_error = true; break; }
                        result->Set(ctx, row_count++, Data::GetRowJS(isolate, ctx, handle, safe_ints, mode)).FromJust();
                }

                if (sqlite3_reset(handle) == SQLITE_OK && !js_error) {
                        db -> GetState ( ) -> busy = false ; info . GetReturnValue ( ) . Set ( result ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
                }
                if (js_error) db->GetState()->was_js_error = true;
                db -> GetState ( ) -> busy = false ; db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
}
#line 208 "./src/objects/statement.lzz"
void Statement::JS_iterate (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 208 "./src/objects/statement.lzz"
                                {
                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8::Local<v8::Function> c = addon->StatementIterator.Get(isolate);
                addon->privileged_info = &info;
                v8::MaybeLocal<v8::Object> maybeIterator = c->NewInstance( isolate -> GetCurrentContext ( ) , 0, NULL);
                addon->privileged_info = NULL;
                if (!maybeIterator.IsEmpty()) info.GetReturnValue().Set(maybeIterator.ToLocalChecked());
}
#line 218 "./src/objects/statement.lzz"
void Statement::JS_bind (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 218 "./src/objects/statement.lzz"
                             {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                if (stmt->bound) return ThrowTypeError("The bind() method can only be invoked once per statement object");
                if ( ! stmt -> db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( stmt -> db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ;
                Binder binder ( stmt -> handle ) ; if ( ! binder . Bind ( info , info . Length ( ) , stmt ) ) { sqlite3_clear_bindings ( stmt -> handle ) ; return ; } ( ( void ) 0 ) ;
                stmt->bound = true;
                info.GetReturnValue().Set(info.This());
}
#line 229 "./src/objects/statement.lzz"
void Statement::JS_pluck (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 229 "./src/objects/statement.lzz"
                              {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                if (!stmt->returns_data) return ThrowTypeError("The pluck() method is only for statements that return data");
                if ( stmt -> db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ;
                bool use = true;
                if (info.Length() != 0) { if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a boolean" ) ; use = ( info [ 0 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ; }
                stmt->mode = use ? Data::PLUCK : stmt->mode == Data::PLUCK ? Data::FLAT : stmt->mode;
                info.GetReturnValue().Set(info.This());
}
#line 240 "./src/objects/statement.lzz"
void Statement::JS_expand (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 240 "./src/objects/statement.lzz"
                               {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                if (!stmt->returns_data) return ThrowTypeError("The expand() method is only for statements that return data");
                if ( stmt -> db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ;
                bool use = true;
                if (info.Length() != 0) { if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a boolean" ) ; use = ( info [ 0 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ; }
                stmt->mode = use ? Data::EXPAND : stmt->mode == Data::EXPAND ? Data::FLAT : stmt->mode;
                info.GetReturnValue().Set(info.This());
}
#line 251 "./src/objects/statement.lzz"
void Statement::JS_raw (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 251 "./src/objects/statement.lzz"
                            {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                if (!stmt->returns_data) return ThrowTypeError("The raw() method is only for statements that return data");
                if ( stmt -> db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ;
                bool use = true;
                if (info.Length() != 0) { if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a boolean" ) ; use = ( info [ 0 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ; }
                stmt->mode = use ? Data::RAW : stmt->mode == Data::RAW ? Data::FLAT : stmt->mode;
                info.GetReturnValue().Set(info.This());
}
#line 262 "./src/objects/statement.lzz"
void Statement::JS_safeIntegers (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 262 "./src/objects/statement.lzz"
                                     {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                if ( stmt -> db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ;
                if (info.Length() == 0) stmt->safe_ints = true;
                else { if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsBoolean ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a boolean" ) ; stmt -> safe_ints = ( info [ 0 ] . As < v8 :: Boolean > ( ) ) -> Value ( ) ; }
                info.GetReturnValue().Set(info.This());
}
#line 271 "./src/objects/statement.lzz"
void Statement::JS_columns (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 271 "./src/objects/statement.lzz"
                                {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                if (!stmt->returns_data) return ThrowTypeError("The columns() method is only for statements that return data");
                if ( ! stmt -> db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( stmt -> db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                Addon* addon = stmt->db->GetAddon();
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;

                int column_count = sqlite3_column_count(stmt->handle);
                v8::Local<v8::Array> columns = v8::Array::New(isolate);

                v8::Local<v8::String> name = addon->cs.name.Get(isolate);
                v8::Local<v8::String> columnName = addon->cs.column.Get(isolate);
                v8::Local<v8::String> tableName = addon->cs.table.Get(isolate);
                v8::Local<v8::String> databaseName = addon->cs.database.Get(isolate);
                v8::Local<v8::String> typeName = addon->cs.type.Get(isolate);

                for (int i = 0; i < column_count; ++i) {
                        v8::Local<v8::Object> column = v8::Object::New(isolate);

                        column->Set(ctx, name,
                                InternalizedFromUtf8OrNull(isolate, sqlite3_column_name(stmt->handle, i), -1)
                        ).FromJust();
                        column->Set(ctx, columnName,
                                InternalizedFromUtf8OrNull(isolate, sqlite3_column_origin_name(stmt->handle, i), -1)
                        ).FromJust();
                        column->Set(ctx, tableName,
                                InternalizedFromUtf8OrNull(isolate, sqlite3_column_table_name(stmt->handle, i), -1)
                        ).FromJust();
                        column->Set(ctx, databaseName,
                                InternalizedFromUtf8OrNull(isolate, sqlite3_column_database_name(stmt->handle, i), -1)
                        ).FromJust();
                        column->Set(ctx, typeName,
                                InternalizedFromUtf8OrNull(isolate, sqlite3_column_decltype(stmt->handle, i), -1)
                        ).FromJust();

                        columns->Set(ctx, i, column).FromJust();
                }

                info.GetReturnValue().Set(columns);
}
#line 314 "./src/objects/statement.lzz"
void Statement::JS_busy (v8::Local <v8 :: String> _, v8::PropertyCallbackInfo <v8 :: Value> const & info)
#line 314 "./src/objects/statement.lzz"
                             {
                Statement* stmt = node :: ObjectWrap :: Unwrap <Statement>(info.This());
                info.GetReturnValue().Set(stmt->alive && stmt->locked);
}
#line 4 "./src/objects/statement-iterator.lzz"
v8::Local <v8 :: Function> StatementIterator::Init (v8::Isolate * isolate, v8::Local <v8 :: External> data)
#line 4 "./src/objects/statement-iterator.lzz"
                   {
                v8::Local<v8::FunctionTemplate> t = NewConstructorTemplate(isolate, data, JS_new, "StatementIterator");
                SetPrototypeMethod(isolate, data, t, "next", JS_next);
                SetPrototypeMethod(isolate, data, t, "return", JS_return);
                SetPrototypeSymbolMethod(isolate, data, t, v8::Symbol::GetIterator(isolate), JS_symbolIterator);
                return t->GetFunction( isolate -> GetCurrentContext ( ) ).ToLocalChecked();
}
#line 15 "./src/objects/statement-iterator.lzz"
StatementIterator::~ StatementIterator ()
#line 15 "./src/objects/statement-iterator.lzz"
                             {}
#line 19 "./src/objects/statement-iterator.lzz"
StatementIterator::StatementIterator (Statement * stmt, bool bound)
#line 19 "./src/objects/statement-iterator.lzz"
  : node::ObjectWrap (), stmt (stmt), handle (stmt->handle), db_state (stmt->db->GetState()), bound (bound), safe_ints (stmt->safe_ints), mode (stmt->mode), alive (true), logged (!db_state->has_logger)
#line 27 "./src/objects/statement-iterator.lzz"
                                              {
                assert(stmt != NULL);
                assert(handle != NULL);
                assert(stmt->bound == bound);
                assert(stmt->alive == true);
                assert(stmt->locked == false);
                assert(db_state->iterators < USHRT_MAX);
                stmt->locked = true;
                db_state->iterators += 1;
}
#line 38 "./src/objects/statement-iterator.lzz"
void StatementIterator::JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 38 "./src/objects/statement-iterator.lzz"
                            {
                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                if (!addon->privileged_info) return ThrowTypeError("Disabled constructor");
                assert(info.IsConstructCall());

                StatementIterator* iter;
                {
                        const v8 :: FunctionCallbackInfo < v8 :: Value > & info = *addon->privileged_info;
                        Statement * stmt = node :: ObjectWrap :: Unwrap < Statement > ( info . This ( ) ) ; if ( ! stmt -> returns_data ) return ThrowTypeError ( "This statement does not return data. Use run() instead" ) ; sqlite3_stmt * handle = stmt -> handle ; Database * db = stmt -> db ; if ( ! db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ; if ( db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ; if ( stmt -> locked ) return ThrowTypeError ( "This statement is busy executing a query" ) ; if ( db -> GetState ( ) -> iterators == USHRT_MAX ) return ThrowRangeError ( "Too many active database iterators" ) ; const bool bound = stmt -> bound ; if ( ! bound ) { Binder binder ( handle ) ; if ( ! binder . Bind ( info , info . Length ( ) , stmt ) ) { sqlite3_clear_bindings ( handle ) ; return ; } ( ( void ) 0 ) ; } else if ( info . Length ( ) > 0 ) { return ThrowTypeError ( "This statement already has bound parameters" ) ; } ( ( void ) 0 ) ;
                        iter = new StatementIterator(stmt, bound);
                }
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                iter->Wrap(info.This());
                SetFrozen(isolate, ctx, info.This(), addon->cs.statement, addon->privileged_info->This());

                info.GetReturnValue().Set(info.This());
}
#line 57 "./src/objects/statement-iterator.lzz"
void StatementIterator::JS_next (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 57 "./src/objects/statement-iterator.lzz"
                             {
                StatementIterator* iter = node :: ObjectWrap :: Unwrap <StatementIterator>(info.This());
                if ( iter -> db_state -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if (iter->alive) iter->Next(info);
                else info.GetReturnValue().Set(DoneRecord( info . GetIsolate ( ) , iter->db_state->addon));
}
#line 64 "./src/objects/statement-iterator.lzz"
void StatementIterator::JS_return (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 64 "./src/objects/statement-iterator.lzz"
                               {
                StatementIterator* iter = node :: ObjectWrap :: Unwrap <StatementIterator>(info.This());
                if ( iter -> db_state -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;
                if (iter->alive) iter->Return(info);
                else info.GetReturnValue().Set(DoneRecord( info . GetIsolate ( ) , iter->db_state->addon));
}
#line 71 "./src/objects/statement-iterator.lzz"
void StatementIterator::JS_symbolIterator (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 71 "./src/objects/statement-iterator.lzz"
                                       {
                info.GetReturnValue().Set(info.This());
}
#line 75 "./src/objects/statement-iterator.lzz"
void StatementIterator::Next (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 75 "./src/objects/statement-iterator.lzz"
                                       {
                assert(alive == true);
                db_state->busy = true;
                if (!logged) {
                        logged = true;
                        if (stmt->db->Log( info . GetIsolate ( ) , handle)) {
                                db_state->busy = false;
                                Throw();
                                return;
                        }
                }
                int status = sqlite3_step(handle);
                db_state->busy = false;
                if (status == SQLITE_ROW) {
                        v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                        v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                        info.GetReturnValue().Set(
                                NewRecord(isolate, ctx, Data::GetRowJS(isolate, ctx, handle, safe_ints, mode), db_state->addon, false)
                        );
                } else {
                        if (status == SQLITE_DONE) Return(info);
                        else Throw();
                }
}
#line 100 "./src/objects/statement-iterator.lzz"
void StatementIterator::Return (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 100 "./src/objects/statement-iterator.lzz"
                                         {
                Cleanup();
                info . GetReturnValue ( ) . Set ( DoneRecord ( info . GetIsolate ( ) , db_state -> addon ) ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
}
#line 105 "./src/objects/statement-iterator.lzz"
void StatementIterator::Throw ()
#line 105 "./src/objects/statement-iterator.lzz"
                     {
                Cleanup();
                Database* db = stmt->db;
                db -> ThrowDatabaseError ( ) ; if ( ! bound ) { sqlite3_clear_bindings ( handle ) ; } return ;
}
#line 111 "./src/objects/statement-iterator.lzz"
void StatementIterator::Cleanup ()
#line 111 "./src/objects/statement-iterator.lzz"
                       {
                assert(alive == true);
                alive = false;
                stmt->locked = false;
                db_state->iterators -= 1;
                sqlite3_reset(handle);
}
#line 4 "./src/objects/backup.lzz"
v8::Local <v8 :: Function> Backup::Init (v8::Isolate * isolate, v8::Local <v8 :: External> data)
#line 4 "./src/objects/backup.lzz"
                   {
                v8::Local<v8::FunctionTemplate> t = NewConstructorTemplate(isolate, data, JS_new, "Backup");
                SetPrototypeMethod(isolate, data, t, "transfer", JS_transfer);
                SetPrototypeMethod(isolate, data, t, "close", JS_close);
                return t->GetFunction( isolate -> GetCurrentContext ( ) ).ToLocalChecked();
}
#line 17 "./src/objects/backup.lzz"
void Backup::CloseHandles ()
#line 17 "./src/objects/backup.lzz"
                            {
                if (alive) {
                        alive = false;
                        std::string filename(sqlite3_db_filename(dest_handle, "main"));
                        sqlite3_backup_finish(backup_handle);
                        int status = sqlite3_close(dest_handle);
                        assert(status == SQLITE_OK); ((void)status);
                        if (unlink) remove(filename.c_str());
                }
}
#line 28 "./src/objects/backup.lzz"
Backup::~ Backup ()
#line 28 "./src/objects/backup.lzz"
                  {
                if (alive) db->RemoveBackup(this);
                CloseHandles();
}
#line 35 "./src/objects/backup.lzz"
Backup::Backup (Database * db, sqlite3 * dest_handle, sqlite3_backup * backup_handle, sqlite3_uint64 id, bool unlink)
#line 41 "./src/objects/backup.lzz"
  : node::ObjectWrap (), db (db), dest_handle (dest_handle), backup_handle (backup_handle), id (id), alive (true), unlink (unlink)
#line 48 "./src/objects/backup.lzz"
                               {
                assert(db != NULL);
                assert(dest_handle != NULL);
                assert(backup_handle != NULL);
                db->AddBackup(this);
}
#line 55 "./src/objects/backup.lzz"
void Backup::JS_new (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 55 "./src/objects/backup.lzz"
                            {
                Addon * addon = static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ;
                if (!addon->privileged_info) return ThrowTypeError("Disabled constructor");
                assert(info.IsConstructCall());
                Database* db = node :: ObjectWrap :: Unwrap <Database>(addon->privileged_info->This());
                if ( ! db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                if ( db -> GetState ( ) -> busy ) return ThrowTypeError ( "This database connection is busy executing a query" ) ;

                v8::Local<v8::Object> database = (*addon->privileged_info)[0].As<v8::Object>();
                v8::Local<v8::String> attachedName = (*addon->privileged_info)[1].As<v8::String>();
                v8::Local<v8::String> destFile = (*addon->privileged_info)[2].As<v8::String>();
                bool unlink = (*addon->privileged_info)[3].As<v8::Boolean>()->Value();

                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                sqlite3* dest_handle;
                v8::String::Utf8Value dest_file(isolate, destFile);
                v8::String::Utf8Value attached_name(isolate, attachedName);
                int mask = (SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE);

                if (sqlite3_open_v2(*dest_file, &dest_handle, mask, NULL) != SQLITE_OK) {
                        Database::ThrowSqliteError(addon, dest_handle);
                        int status = sqlite3_close(dest_handle);
                        assert(status == SQLITE_OK); ((void)status);
                        return;
                }

                sqlite3_extended_result_codes(dest_handle, 1);
                sqlite3_limit(dest_handle, SQLITE_LIMIT_LENGTH, INT_MAX);
                sqlite3_backup* backup_handle = sqlite3_backup_init(dest_handle, "main", db->GetHandle(), *attached_name);
                if (backup_handle == NULL) {
                        Database::ThrowSqliteError(addon, dest_handle);
                        int status = sqlite3_close(dest_handle);
                        assert(status == SQLITE_OK); ((void)status);
                        return;
                }

                Backup* backup = new Backup(db, dest_handle, backup_handle, addon->NextId(), unlink);
                backup->Wrap(info.This());
                SetFrozen(isolate, isolate -> GetCurrentContext ( ) , info.This(), addon->cs.database, database);

                info.GetReturnValue().Set(info.This());
}
#line 98 "./src/objects/backup.lzz"
void Backup::JS_transfer (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 98 "./src/objects/backup.lzz"
                                 {
                Backup* backup = node :: ObjectWrap :: Unwrap <Backup>(info.This());
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsInt32 ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a 32-bit signed integer" ) ; int pages = ( info [ 0 ] . As < v8 :: Int32 > ( ) ) -> Value ( ) ;
                if ( ! backup -> db -> GetState ( ) -> open ) return ThrowTypeError ( "The database connection is not open" ) ;
                assert(backup->db->GetState()->busy == false);
                assert(backup->alive == true);

                sqlite3_backup* backup_handle = backup->backup_handle;
                int status = sqlite3_backup_step(backup_handle, pages) & 0xff;

                Addon* addon = backup->db->GetAddon();
                if (status == SQLITE_OK || status == SQLITE_DONE || status == SQLITE_BUSY) {
                        int total_pages = sqlite3_backup_pagecount(backup_handle);
                        int remaining_pages = sqlite3_backup_remaining(backup_handle);
                        v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                        v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                        v8::Local<v8::Object> result = v8::Object::New(isolate);
                        result->Set(ctx, addon->cs.totalPages.Get(isolate), v8::Int32::New(isolate, total_pages)).FromJust();
                        result->Set(ctx, addon->cs.remainingPages.Get(isolate), v8::Int32::New(isolate, remaining_pages)).FromJust();
                        info.GetReturnValue().Set(result);
                        if (status == SQLITE_DONE) backup->unlink = false;
                } else {
                        Database::ThrowSqliteError(addon, sqlite3_errstr(status), status);
                }
}
#line 124 "./src/objects/backup.lzz"
void Backup::JS_close (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 124 "./src/objects/backup.lzz"
                              {
                Backup* backup = node :: ObjectWrap :: Unwrap <Backup>(info.This());
                assert(backup->db->GetState()->busy == false);
                if (backup->alive) backup->db->RemoveBackup(backup);
                backup->CloseHandles();
                info.GetReturnValue().Set(info.This());
}
#line 4 "./src/util/data-converter.lzz"
void DataConverter::ThrowDataConversionError (sqlite3_context * invocation, bool isBigInt)
#line 4 "./src/util/data-converter.lzz"
                                                                                  {
                if (isBigInt) {
                        ThrowRangeError((GetDataErrorPrefix() + " a bigint that was too big").c_str());
                } else {
                        ThrowTypeError((GetDataErrorPrefix() + " an invalid value").c_str());
                }
                PropagateJSError(invocation);
}
#line 4 "./src/util/custom-function.lzz"
CustomFunction::CustomFunction (v8::Isolate * isolate, Database * db, char const * name, v8::Local <v8::Function> fn, bool safe_ints)
#line 10 "./src/util/custom-function.lzz"
  : name (name), db (db), isolate (isolate), fn (isolate, fn), safe_ints (safe_ints)
#line 15 "./src/util/custom-function.lzz"
                                     {}
#line 17 "./src/util/custom-function.lzz"
CustomFunction::~ CustomFunction ()
#line 17 "./src/util/custom-function.lzz"
                                  {}
#line 19 "./src/util/custom-function.lzz"
void CustomFunction::xDestroy (void * self)
#line 19 "./src/util/custom-function.lzz"
                                         {
                delete static_cast<CustomFunction*>(self);
}
#line 23 "./src/util/custom-function.lzz"
void CustomFunction::xFunc (sqlite3_context * invocation, int argc, sqlite3_value * * argv)
#line 23 "./src/util/custom-function.lzz"
                                                                                       {
                CustomFunction * self = static_cast < CustomFunction * > ( sqlite3_user_data ( invocation ) ) ; v8 :: Isolate * isolate = self -> isolate ; v8 :: HandleScope scope ( isolate ) ;

                v8::Local<v8::Value> args_fast[4];
                v8::Local<v8::Value>* args = NULL;
                if (argc != 0) {
                        args = argc <= 4 ? args_fast : ALLOC_ARRAY<v8::Local<v8::Value>>(argc);
                        Data::GetArgumentsJS(isolate, args, argv, argc, self->safe_ints);
                }

                v8::MaybeLocal<v8::Value> maybeReturnValue = self->fn.Get(isolate)->Call( isolate -> GetCurrentContext ( ) , v8::Undefined(isolate), argc, args);
                if (args != args_fast) delete[] args;

                if (maybeReturnValue.IsEmpty()) self->PropagateJSError(invocation);
                else Data::ResultValueFromJS(isolate, invocation, maybeReturnValue.ToLocalChecked(), self);
}
#line 42 "./src/util/custom-function.lzz"
void CustomFunction::PropagateJSError (sqlite3_context * invocation)
#line 42 "./src/util/custom-function.lzz"
                                                           {
                assert(db->GetState()->was_js_error == false);
                db->GetState()->was_js_error = true;
                sqlite3_result_error(invocation, "", 0);
}
#line 48 "./src/util/custom-function.lzz"
std::string CustomFunction::GetDataErrorPrefix ()
#line 48 "./src/util/custom-function.lzz"
                                         {
                return std::string("User-defined function ") + name + "() returned";
}
#line 4 "./src/util/custom-aggregate.lzz"
CustomAggregate::CustomAggregate (v8::Isolate * isolate, Database * db, char const * name, v8::Local <v8::Value> start, v8::Local <v8::Function> step, v8::Local <v8::Value> inverse, v8::Local <v8::Value> result, bool safe_ints)
#line 13 "./src/util/custom-aggregate.lzz"
  : CustomFunction (isolate, db, name, step, safe_ints), invoke_result (result->IsFunction()), invoke_start (start->IsFunction()), inverse (isolate, inverse->IsFunction() ? inverse.As<v8::Function>() : v8::Local<v8::Function>()), result (isolate, result->IsFunction() ? result.As<v8::Function>() : v8::Local<v8::Function>()), start (isolate, start)
#line 19 "./src/util/custom-aggregate.lzz"
                                      {}
#line 21 "./src/util/custom-aggregate.lzz"
void CustomAggregate::xStep (sqlite3_context * invocation, int argc, sqlite3_value * * argv)
#line 21 "./src/util/custom-aggregate.lzz"
                                                                                       {
                xStepBase(invocation, argc, argv, &CustomAggregate::fn);
}
#line 25 "./src/util/custom-aggregate.lzz"
void CustomAggregate::xInverse (sqlite3_context * invocation, int argc, sqlite3_value * * argv)
#line 25 "./src/util/custom-aggregate.lzz"
                                                                                          {
                xStepBase(invocation, argc, argv, &CustomAggregate::inverse);
}
#line 29 "./src/util/custom-aggregate.lzz"
void CustomAggregate::xValue (sqlite3_context * invocation)
#line 29 "./src/util/custom-aggregate.lzz"
                                                        {
                xValueBase(invocation, false);
}
#line 33 "./src/util/custom-aggregate.lzz"
void CustomAggregate::xFinal (sqlite3_context * invocation)
#line 33 "./src/util/custom-aggregate.lzz"
                                                        {
                xValueBase(invocation, true);
}
#line 88 "./src/util/custom-aggregate.lzz"
CustomAggregate::Accumulator * CustomAggregate::GetAccumulator (sqlite3_context * invocation)
#line 88 "./src/util/custom-aggregate.lzz"
                                                                 {
                Accumulator* acc = static_cast<Accumulator*>(sqlite3_aggregate_context(invocation, sizeof(Accumulator)));
                if (!acc->initialized) {
                        assert(acc->value.IsEmpty());
                        acc->initialized = true;
                        if (invoke_start) {
                                v8::MaybeLocal<v8::Value> maybeSeed = start.Get(isolate).As<v8::Function>()->Call( isolate -> GetCurrentContext ( ) , v8::Undefined(isolate), 0, NULL);
                                if (maybeSeed.IsEmpty()) PropagateJSError(invocation);
                                else acc->value.Reset(isolate, maybeSeed.ToLocalChecked());
                        } else {
                                assert(!start.IsEmpty());
                                acc->value.Reset(isolate, start);
                        }
                }
                return acc;
}
#line 105 "./src/util/custom-aggregate.lzz"
void CustomAggregate::DestroyAccumulator (sqlite3_context * invocation)
#line 105 "./src/util/custom-aggregate.lzz"
                                                                    {
                Accumulator* acc = static_cast<Accumulator*>(sqlite3_aggregate_context(invocation, sizeof(Accumulator)));
                assert(acc->initialized);
                acc->value.Reset();
}
#line 111 "./src/util/custom-aggregate.lzz"
void CustomAggregate::PropagateJSError (sqlite3_context * invocation)
#line 111 "./src/util/custom-aggregate.lzz"
                                                           {
                DestroyAccumulator(invocation);
                CustomFunction::PropagateJSError(invocation);
}
#line 4 "./src/util/custom-table.lzz"
CustomTable::CustomTable (v8::Isolate * isolate, Database * db, char const * name, v8::Local <v8::Function> factory)
#line 9 "./src/util/custom-table.lzz"
  : addon (db->GetAddon()), isolate (isolate), db (db), name (name), factory (isolate, factory)
#line 14 "./src/util/custom-table.lzz"
                                          {}
#line 16 "./src/util/custom-table.lzz"
void CustomTable::Destructor (void * self)
#line 16 "./src/util/custom-table.lzz"
                                           {
                delete static_cast<CustomTable*>(self);
}
#line 20 "./src/util/custom-table.lzz"
sqlite3_module CustomTable::MODULE = {
                0,
                xCreate,
                xConnect,
                xBestIndex,
                xDisconnect,
                xDisconnect,
                xOpen,
                xClose,
                xFilter,
                xNext,
                xEof,
                xColumn,
                xRowid,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL
        };
#line 47 "./src/util/custom-table.lzz"
sqlite3_module CustomTable::EPONYMOUS_MODULE = {
                0,
                NULL,
                xConnect,
                xBestIndex,
                xDisconnect,
                xDisconnect,
                xOpen,
                xClose,
                xFilter,
                xNext,
                xEof,
                xColumn,
                xRowid,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL,
                NULL
        };
#line 78 "./src/util/custom-table.lzz"
CustomTable::VTab::VTab (CustomTable * parent, v8::Local <v8::Function> generator, std::vector <std::string> parameter_names, bool safe_ints)
#line 83 "./src/util/custom-table.lzz"
  : parent (parent), parameter_count (parameter_names.size()), safe_ints (safe_ints), generator (parent->isolate, generator), parameter_names (parameter_names)
#line 88 "./src/util/custom-table.lzz"
                                                         {
                        ((void)base);
}
#line 132 "./src/util/custom-table.lzz"
CustomTable::TempDataConverter::TempDataConverter (CustomTable * parent)
#line 132 "./src/util/custom-table.lzz"
  : parent (parent), status (SQLITE_OK)
#line 134 "./src/util/custom-table.lzz"
                                          {}
#line 136 "./src/util/custom-table.lzz"
void CustomTable::TempDataConverter::PropagateJSError (sqlite3_context * invocation)
#line 136 "./src/util/custom-table.lzz"
                                                                   {
                        status = SQLITE_ERROR;
                        parent->PropagateJSError();
}
#line 141 "./src/util/custom-table.lzz"
std::string CustomTable::TempDataConverter::GetDataErrorPrefix ()
#line 141 "./src/util/custom-table.lzz"
                                                 {
                        return std::string("Virtual table module \"") + parent->name + "\" yielded";
}
#line 151 "./src/util/custom-table.lzz"
int CustomTable::xCreate (sqlite3 * db_handle, void * _self, int argc, char const * const * argv, sqlite3_vtab * * output, char * * errOutput)
#line 151 "./src/util/custom-table.lzz"
                                                                                                                                         {
                return xConnect(db_handle, _self, argc, argv, output, errOutput);
}
#line 156 "./src/util/custom-table.lzz"
int CustomTable::xConnect (sqlite3 * db_handle, void * _self, int argc, char const * const * argv, sqlite3_vtab * * output, char * * errOutput)
#line 156 "./src/util/custom-table.lzz"
                                                                                                                                          {
                CustomTable* self = static_cast<CustomTable*>(_self);
                v8::Isolate* isolate = self->isolate;
                v8::HandleScope scope(isolate);
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;

                v8::Local<v8::Value>* args = ALLOC_ARRAY<v8::Local<v8::Value>>(argc);
                for (int i = 0; i < argc; ++i) {
                        args[i] = StringFromUtf8(isolate, argv[i], -1);
                }


                v8::MaybeLocal<v8::Value> maybeReturnValue = self->factory.Get(isolate)->Call(ctx, v8::Undefined(isolate), argc, args);
                delete[] args;

                if (maybeReturnValue.IsEmpty()) {
                        self->PropagateJSError();
                        return SQLITE_ERROR;
                }


                v8::Local<v8::Array> returnValue = maybeReturnValue.ToLocalChecked().As<v8::Array>();
                v8::Local<v8::String> sqlString = returnValue->Get(ctx, 0).ToLocalChecked().As<v8::String>();
                v8::Local<v8::Function> generator = returnValue->Get(ctx, 1).ToLocalChecked().As<v8::Function>();
                v8::Local<v8::Array> parameterNames = returnValue->Get(ctx, 2).ToLocalChecked().As<v8::Array>();
                int safe_ints = returnValue->Get(ctx, 3).ToLocalChecked().As<v8::Int32>()->Value();
                bool direct_only = returnValue->Get(ctx, 4).ToLocalChecked().As<v8::Boolean>()->Value();

                v8::String::Utf8Value sql(isolate, sqlString);
                safe_ints = safe_ints < 2 ? safe_ints : static_cast<int>(self->db->GetState()->safe_ints);


                std::vector<std::string> parameter_names;
                for (int i = 0, len = parameterNames->Length(); i < len; ++i) {
                        v8::Local<v8::String> parameterName = parameterNames->Get(ctx, i).ToLocalChecked().As<v8::String>();
                        v8::String::Utf8Value parameter_name(isolate, parameterName);
                        parameter_names.emplace_back(*parameter_name);
                }


                if (sqlite3_declare_vtab(db_handle, *sql) != SQLITE_OK) {
                        *errOutput = sqlite3_mprintf("failed to declare virtual table \"%s\"", argv[2]);
                        return SQLITE_ERROR;
                }
                if (direct_only && sqlite3_vtab_config(db_handle, SQLITE_VTAB_DIRECTONLY) != SQLITE_OK) {
                        *errOutput = sqlite3_mprintf("failed to configure virtual table \"%s\"", argv[2]);
                        return SQLITE_ERROR;
                }


                *output = (new VTab(self, generator, parameter_names, safe_ints))->Downcast();
                return SQLITE_OK;
}
#line 210 "./src/util/custom-table.lzz"
int CustomTable::xDisconnect (sqlite3_vtab * vtab)
#line 210 "./src/util/custom-table.lzz"
                                                   {
                delete VTab::Upcast(vtab);
                return SQLITE_OK;
}
#line 215 "./src/util/custom-table.lzz"
int CustomTable::xOpen (sqlite3_vtab * vtab, sqlite3_vtab_cursor * * output)
#line 215 "./src/util/custom-table.lzz"
                                                                           {
                *output = (new Cursor())->Downcast();
                return SQLITE_OK;
}
#line 220 "./src/util/custom-table.lzz"
int CustomTable::xClose (sqlite3_vtab_cursor * cursor)
#line 220 "./src/util/custom-table.lzz"
                                                       {
                delete Cursor::Upcast(cursor);
                return SQLITE_OK;
}
#line 228 "./src/util/custom-table.lzz"
int CustomTable::xFilter (sqlite3_vtab_cursor * _cursor, int idxNum, char const * idxStr, int argc, sqlite3_value * * argv)
#line 228 "./src/util/custom-table.lzz"
                                                                                                                         {
                Cursor* cursor = Cursor::Upcast(_cursor);
                VTab* vtab = cursor->GetVTab();
                CustomTable* self = vtab->parent;
                Addon* addon = self->addon;
                v8::Isolate* isolate = self->isolate;
                v8::HandleScope scope(isolate);
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;



                v8::Local<v8::Value> args_fast[4];
                v8::Local<v8::Value>* args = NULL;
                int parameter_count = vtab->parameter_count;
                if (parameter_count != 0) {
                        args = parameter_count <= 4 ? args_fast : ALLOC_ARRAY<v8::Local<v8::Value>>(parameter_count);
                        int argn = 0;
                        bool safe_ints = vtab->safe_ints;
                        for (int i = 0; i < parameter_count; ++i) {
                                if (idxNum & 1 << i) {
                                        args[i] = Data::GetValueJS(isolate, argv[argn++], safe_ints);


                                        if (args[i]->IsNull()) {
                                                if (args != args_fast) delete[] args;
                                                cursor->done = true;
                                                return SQLITE_OK;
                                        }
                                } else {
                                        args[i] = v8::Undefined(isolate);
                                }
                        }
                }


                v8::MaybeLocal<v8::Value> maybeIterator = vtab->generator.Get(isolate)->Call(ctx, v8::Undefined(isolate), parameter_count, args);
                if (args != args_fast) delete[] args;

                if (maybeIterator.IsEmpty()) {
                        self->PropagateJSError();
                        return SQLITE_ERROR;
                }


                v8::Local<v8::Object> iterator = maybeIterator.ToLocalChecked().As<v8::Object>();
                v8::Local<v8::Function> next = iterator->Get(ctx, addon->cs.next.Get(isolate)).ToLocalChecked().As<v8::Function>();
                cursor->iterator.Reset(isolate, iterator);
                cursor->next.Reset(isolate, next);
                cursor->rowid = 0;


                return xNext(cursor->Downcast());
}
#line 284 "./src/util/custom-table.lzz"
int CustomTable::xNext (sqlite3_vtab_cursor * _cursor)
#line 284 "./src/util/custom-table.lzz"
                                                       {
                Cursor* cursor = Cursor::Upcast(_cursor);
                CustomTable* self = cursor->GetVTab()->parent;
                Addon* addon = self->addon;
                v8::Isolate* isolate = self->isolate;
                v8::HandleScope scope(isolate);
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;

                v8::Local<v8::Object> iterator = cursor->iterator.Get(isolate);
                v8::Local<v8::Function> next = cursor->next.Get(isolate);

                v8::MaybeLocal<v8::Value> maybeRecord = next->Call(ctx, iterator, 0, NULL);
                if (maybeRecord.IsEmpty()) {
                        self->PropagateJSError();
                        return SQLITE_ERROR;
                }

                v8::Local<v8::Object> record = maybeRecord.ToLocalChecked().As<v8::Object>();
                bool done = record->Get(ctx, addon->cs.done.Get(isolate)).ToLocalChecked().As<v8::Boolean>()->Value();
                if (!done) {
                        cursor->row.Reset(isolate, record->Get(ctx, addon->cs.value.Get(isolate)).ToLocalChecked().As<v8::Array>());
                }
                cursor->done = done;
                cursor->rowid += 1;

                return SQLITE_OK;
}
#line 313 "./src/util/custom-table.lzz"
int CustomTable::xEof (sqlite3_vtab_cursor * cursor)
#line 313 "./src/util/custom-table.lzz"
                                                     {
                return Cursor::Upcast(cursor)->done;
}
#line 318 "./src/util/custom-table.lzz"
int CustomTable::xColumn (sqlite3_vtab_cursor * _cursor, sqlite3_context * invocation, int column)
#line 318 "./src/util/custom-table.lzz"
                                                                                                  {
                Cursor* cursor = Cursor::Upcast(_cursor);
                CustomTable* self = cursor->GetVTab()->parent;
                TempDataConverter temp_data_converter(self);
                v8::Isolate* isolate = self->isolate;
                v8::HandleScope scope(isolate);

                v8::Local<v8::Array> row = cursor->row.Get(isolate);
                v8::MaybeLocal<v8::Value> maybeColumnValue = row->Get( isolate -> GetCurrentContext ( ) , column);
                if (maybeColumnValue.IsEmpty()) {
                        temp_data_converter.PropagateJSError(NULL);
                } else {
                        Data::ResultValueFromJS(isolate, invocation, maybeColumnValue.ToLocalChecked(), &temp_data_converter);
                }
                return temp_data_converter.status;
}
#line 336 "./src/util/custom-table.lzz"
int CustomTable::xRowid (sqlite3_vtab_cursor * cursor, sqlite_int64 * output)
#line 336 "./src/util/custom-table.lzz"
                                                                             {
                *output = Cursor::Upcast(cursor)->rowid;
                return SQLITE_OK;
}
#line 343 "./src/util/custom-table.lzz"
int CustomTable::xBestIndex (sqlite3_vtab * vtab, sqlite3_index_info * output)
#line 343 "./src/util/custom-table.lzz"
                                                                              {
                int parameter_count = VTab::Upcast(vtab)->parameter_count;
                int argument_count = 0;
                std::vector<std::pair<int, int>> forwarded;

                for (int i = 0, len = output->nConstraint; i < len; ++i) {
                        auto item = output->aConstraint[i];


                        if (item.iColumn >= 0 && item.iColumn < parameter_count) {
                                if (item.op != SQLITE_INDEX_CONSTRAINT_EQ) {
                                        sqlite3_free(vtab->zErrMsg);
                                        vtab->zErrMsg = sqlite3_mprintf(
                                                "virtual table parameter \"%s\" can only be constrained by the '=' operator",
                                                VTab::Upcast(vtab)->parameter_names.at(item.iColumn).c_str());
                                        return SQLITE_ERROR;
                                }
                                if (!item.usable) {



                                        return SQLITE_CONSTRAINT;
                                }
                                forwarded.emplace_back(item.iColumn, i);
                        }
                }


                std::sort(forwarded.begin(), forwarded.end());
                for (std::pair<int, int> pair : forwarded) {
                        int bit = 1 << pair.first;
                        if (!(output->idxNum & bit)) {
                                output->idxNum |= bit;
                                output->aConstraintUsage[pair.second].argvIndex = ++argument_count;
                                output->aConstraintUsage[pair.second].omit = 1;
                        }
                }



                output->estimatedCost = output->estimatedRows = 1000000000 / (argument_count + 1);
                return SQLITE_OK;
}
#line 387 "./src/util/custom-table.lzz"
void CustomTable::PropagateJSError ()
#line 387 "./src/util/custom-table.lzz"
                                {
                assert(db->GetState()->was_js_error == false);
                db->GetState()->was_js_error = true;
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 72 "./src/util/data.lzz"
  v8::Local <v8::Value> GetValueJS (v8::Isolate * isolate, sqlite3_stmt * handle, int column, bool safe_ints)
#line 72 "./src/util/data.lzz"
                                                                                                                {
                switch ( sqlite3_column_type ( handle , column ) ) { case SQLITE_INTEGER : if ( safe_ints ) { return v8 :: BigInt :: New ( isolate , sqlite3_column_int64 ( handle , column ) ) ; } case SQLITE_FLOAT : return v8 :: Number :: New ( isolate , sqlite3_column_double ( handle , column ) ) ; case SQLITE_TEXT : return StringFromUtf8 ( isolate , reinterpret_cast < const char * > ( sqlite3_column_text ( handle , column ) ) , sqlite3_column_bytes ( handle , column ) ) ; case SQLITE_BLOB : return node :: Buffer :: Copy ( isolate , static_cast < const char * > ( sqlite3_column_blob ( handle , column ) ) , sqlite3_column_bytes ( handle , column ) ) . ToLocalChecked ( ) ; default : assert ( sqlite3_column_type ( handle , column ) == SQLITE_NULL ) ; return v8 :: Null ( isolate ) ; } assert ( false ) ; ;
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 76 "./src/util/data.lzz"
  v8::Local <v8::Value> GetValueJS (v8::Isolate * isolate, sqlite3_value * value, bool safe_ints)
#line 76 "./src/util/data.lzz"
                                                                                                    {
                switch ( sqlite3_value_type ( value ) ) { case SQLITE_INTEGER : if ( safe_ints ) { return v8 :: BigInt :: New ( isolate , sqlite3_value_int64 ( value ) ) ; } case SQLITE_FLOAT : return v8 :: Number :: New ( isolate , sqlite3_value_double ( value ) ) ; case SQLITE_TEXT : return StringFromUtf8 ( isolate , reinterpret_cast < const char * > ( sqlite3_value_text ( value ) ) , sqlite3_value_bytes ( value ) ) ; case SQLITE_BLOB : return node :: Buffer :: Copy ( isolate , static_cast < const char * > ( sqlite3_value_blob ( value ) ) , sqlite3_value_bytes ( value ) ) . ToLocalChecked ( ) ; default : assert ( sqlite3_value_type ( value ) == SQLITE_NULL ) ; return v8 :: Null ( isolate ) ; } assert ( false ) ; ;
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 80 "./src/util/data.lzz"
  v8::Local <v8::Value> GetFlatRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints)
#line 80 "./src/util/data.lzz"
                                                                                                                                  {
                v8::Local<v8::Object> row = v8::Object::New(isolate);
                int column_count = sqlite3_column_count(handle);
                for (int i = 0; i < column_count; ++i) {
                        row->Set(ctx,
                                InternalizedFromUtf8(isolate, sqlite3_column_name(handle, i), -1),
                                Data::GetValueJS(isolate, handle, i, safe_ints)).FromJust();
                }
                return row;
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 91 "./src/util/data.lzz"
  v8::Local <v8::Value> GetExpandedRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints)
#line 91 "./src/util/data.lzz"
                                                                                                                                      {
                v8::Local<v8::Object> row = v8::Object::New(isolate);
                int column_count = sqlite3_column_count(handle);
                for (int i = 0; i < column_count; ++i) {
                        const char* table_raw = sqlite3_column_table_name(handle, i);
                        v8::Local<v8::String> table = InternalizedFromUtf8(isolate, table_raw == NULL ? "$" : table_raw, -1);
                        v8::Local<v8::String> column = InternalizedFromUtf8(isolate, sqlite3_column_name(handle, i), -1);
                        v8::Local<v8::Value> value = Data::GetValueJS(isolate, handle, i, safe_ints);
                        if (row->HasOwnProperty(ctx, table).FromJust()) {
                                row->Get(ctx, table).ToLocalChecked().As<v8::Object>()->Set(ctx, column, value).FromJust();
                        } else {
                                v8::Local<v8::Object> nested = v8::Object::New(isolate);
                                row->Set(ctx, table, nested).FromJust();
                                nested->Set(ctx, column, value).FromJust();
                        }
                }
                return row;
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 110 "./src/util/data.lzz"
  v8::Local <v8::Value> GetRawRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints)
#line 110 "./src/util/data.lzz"
                                                                                                                                 {
                v8::Local<v8::Array> row = v8::Array::New(isolate);
                int column_count = sqlite3_column_count(handle);
                for (int i = 0; i < column_count; ++i) {
                        row->Set(ctx, i, Data::GetValueJS(isolate, handle, i, safe_ints)).FromJust();
                }
                return row;
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 119 "./src/util/data.lzz"
  v8::Local <v8::Value> GetRowJS (v8::Isolate * isolate, v8::Local <v8::Context> ctx, sqlite3_stmt * handle, bool safe_ints, char mode)
#line 119 "./src/util/data.lzz"
                                                                                                                                         {
                if (mode == FLAT) return GetFlatRowJS(isolate, ctx, handle, safe_ints);
                if (mode == PLUCK) return GetValueJS(isolate, handle, 0, safe_ints);
                if (mode == EXPAND) return GetExpandedRowJS(isolate, ctx, handle, safe_ints);
                if (mode == RAW) return GetRawRowJS(isolate, ctx, handle, safe_ints);
                assert(false);
                return v8::Local<v8::Value>();
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 128 "./src/util/data.lzz"
  void GetArgumentsJS (v8::Isolate * isolate, v8::Local <v8::Value> * out, sqlite3_value * * values, int argument_count, bool safe_ints)
#line 128 "./src/util/data.lzz"
                                                                                                                                         {
                assert(argument_count > 0);
                for (int i = 0; i < argument_count; ++i) {
                        out[i] = Data::GetValueJS(isolate, values[i], safe_ints);
                }
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 135 "./src/util/data.lzz"
  int BindValueFromJS (v8::Isolate * isolate, sqlite3_stmt * handle, int index, v8::Local <v8::Value> value)
#line 135 "./src/util/data.lzz"
                                                                                                               {
                if ( value -> IsNumber ( ) ) { return sqlite3_bind_double ( handle , index , value . As < v8 :: Number > ( ) -> Value ( ) ) ; } else if ( value -> IsBigInt ( ) ) { bool lossless ; int64_t v = value . As < v8 :: BigInt > ( ) -> Int64Value ( & lossless ) ; if ( lossless ) { return sqlite3_bind_int64 ( handle , index , v ) ; } } else if ( value -> IsString ( ) ) { v8 :: String :: Utf8Value utf8 ( isolate , value . As < v8 :: String > ( ) ) ; return sqlite3_bind_text ( handle , index , * utf8 , utf8 . length ( ) , SQLITE_TRANSIENT ) ; } else if ( node :: Buffer :: HasInstance ( value ) ) { const char * data = node :: Buffer :: Data ( value ) ; return sqlite3_bind_blob ( handle , index , data ? data : "" , node :: Buffer :: Length ( value ) , SQLITE_TRANSIENT ) ; } else if ( value -> IsNull ( ) || value -> IsUndefined ( ) ) { return sqlite3_bind_null ( handle , index ) ; } ;
                return value->IsBigInt() ? SQLITE_TOOBIG : -1;
  }
}
#line 65 "./src/util/data.lzz"
namespace Data
{
#line 140 "./src/util/data.lzz"
  void ResultValueFromJS (v8::Isolate * isolate, sqlite3_context * invocation, v8::Local <v8::Value> value, DataConverter * converter)
#line 140 "./src/util/data.lzz"
                                                                                                                                        {
                if ( value -> IsNumber ( ) ) { return sqlite3_result_double ( invocation , value . As < v8 :: Number > ( ) -> Value ( ) ) ; } else if ( value -> IsBigInt ( ) ) { bool lossless ; int64_t v = value . As < v8 :: BigInt > ( ) -> Int64Value ( & lossless ) ; if ( lossless ) { return sqlite3_result_int64 ( invocation , v ) ; } } else if ( value -> IsString ( ) ) { v8 :: String :: Utf8Value utf8 ( isolate , value . As < v8 :: String > ( ) ) ; return sqlite3_result_text ( invocation , * utf8 , utf8 . length ( ) , SQLITE_TRANSIENT ) ; } else if ( node :: Buffer :: HasInstance ( value ) ) { const char * data = node :: Buffer :: Data ( value ) ; return sqlite3_result_blob ( invocation , data ? data : "" , node :: Buffer :: Length ( value ) , SQLITE_TRANSIENT ) ; } else if ( value -> IsNull ( ) || value -> IsUndefined ( ) ) { return sqlite3_result_null ( invocation ) ; } ;
                converter->ThrowDataConversionError(invocation, value->IsBigInt());
  }
}
#line 4 "./src/util/binder.lzz"
Binder::Binder (sqlite3_stmt * _handle)
#line 4 "./src/util/binder.lzz"
                                               {
                handle = _handle;
                param_count = sqlite3_bind_parameter_count(_handle);
                anon_index = 0;
                success = true;
}
#line 11 "./src/util/binder.lzz"
bool Binder::Bind (v8::FunctionCallbackInfo <v8 :: Value> const & info, int argc, Statement * stmt)
#line 11 "./src/util/binder.lzz"
                                                                  {
                assert(anon_index == 0);
                Result result = BindArgs(info, argc, stmt);
                if (success && result.count != param_count) {
                        if (result.count < param_count) {
                                if (!result.bound_object && stmt->GetBindMap( info . GetIsolate ( ) )->GetSize()) {
                                        Fail(ThrowTypeError, "Missing named parameters");
                                } else {
                                        Fail(ThrowRangeError, "Too few parameter values were provided");
                                }
                        } else {
                                Fail(ThrowRangeError, "Too many parameter values were provided");
                        }
                }
                return success;
}
#line 35 "./src/util/binder.lzz"
bool Binder::IsPlainObject (v8::Isolate * isolate, v8::Local <v8::Object> obj)
#line 35 "./src/util/binder.lzz"
                                                                                   {
                v8::Local<v8::Value> proto = obj->GetPrototype();
                v8::Local<v8::Context> ctx = obj->CreationContext();
                ctx->Enter();
                v8::Local<v8::Value> baseProto = v8::Object::New(isolate)->GetPrototype();
                ctx->Exit();
                return proto->StrictEquals(baseProto) || proto->StrictEquals(v8::Null(isolate));
}
#line 44 "./src/util/binder.lzz"
void Binder::Fail (void (* Throw) (char const *), char const * message)
#line 44 "./src/util/binder.lzz"
                                                                     {
                assert(success == true);
                assert((Throw == NULL) == (message == NULL));
                assert(Throw == ThrowError || Throw == ThrowTypeError || Throw == ThrowRangeError || Throw == NULL);
                if (Throw) Throw(message);
                success = false;
}
#line 52 "./src/util/binder.lzz"
int Binder::NextAnonIndex ()
#line 52 "./src/util/binder.lzz"
                            {
                while (sqlite3_bind_parameter_name(handle, ++anon_index) != NULL) {}
                return anon_index;
}
#line 58 "./src/util/binder.lzz"
void Binder::BindValue (v8::Isolate * isolate, v8::Local <v8::Value> value, int index)
#line 58 "./src/util/binder.lzz"
                                                                                    {
                int status = Data::BindValueFromJS(isolate, handle, index, value);
                if (status != SQLITE_OK) {
                        switch (status) {
                                case -1:
                                        return Fail(ThrowTypeError, "SQLite3 can only bind numbers, strings, bigints, buffers, and null");
                                case SQLITE_TOOBIG:
                                        return Fail(ThrowRangeError, "The bound string, buffer, or bigint is too big");
                                case SQLITE_RANGE:
                                        return Fail(ThrowRangeError, "Too many parameter values were provided");
                                case SQLITE_NOMEM:
                                        return Fail(ThrowError, "Out of memory");
                                default:
                                        return Fail(ThrowError, "An unexpected error occured while trying to bind parameters");
                        }
                        assert(false);
                }
}
#line 79 "./src/util/binder.lzz"
int Binder::BindArray (v8::Isolate * isolate, v8::Local <v8::Array> arr)
#line 79 "./src/util/binder.lzz"
                                                                      {
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                uint32_t length = arr->Length();
                if (length > INT_MAX) {
                        Fail(ThrowRangeError, "Too many parameter values were provided");
                        return 0;
                }
                int len = static_cast<int>(length);
                for (int i = 0; i < len; ++i) {
                        v8::MaybeLocal<v8::Value> maybeValue = arr->Get(ctx, i);
                        if (maybeValue.IsEmpty()) {
                                Fail(NULL, NULL);
                                return i;
                        }
                        BindValue(isolate, maybeValue.ToLocalChecked(), NextAnonIndex());
                        if (!success) {
                                return i;
                        }
                }
                return len;
}
#line 105 "./src/util/binder.lzz"
int Binder::BindObject (v8::Isolate * isolate, v8::Local <v8::Object> obj, Statement * stmt)
#line 105 "./src/util/binder.lzz"
                                                                                         {
                v8 :: Local < v8 :: Context > ctx = isolate -> GetCurrentContext ( ) ;
                BindMap* bind_map = stmt->GetBindMap(isolate);
                BindMap::Pair* pairs = bind_map->GetPairs();
                int len = bind_map->GetSize();

                for (int i = 0; i < len; ++i) {
                        v8::Local<v8::String> key = pairs[i].GetName(isolate);


                        v8::Maybe<bool> has_property = obj->HasOwnProperty(ctx, key);
                        if (has_property.IsNothing()) {
                                Fail(NULL, NULL);
                                return i;
                        }
                        if (!has_property.FromJust()) {
                                v8::String::Utf8Value param_name(isolate, key);
                                Fail(ThrowRangeError, (std::string("Missing named parameter \"") + *param_name + "\"").c_str());
                                return i;
                        }


                        v8::MaybeLocal<v8::Value> maybeValue = obj->Get(ctx, key);
                        if (maybeValue.IsEmpty()) {
                                Fail(NULL, NULL);
                                return i;
                        }

                        BindValue(isolate, maybeValue.ToLocalChecked(), pairs[i].GetIndex());
                        if (!success) {
                                return i;
                        }
                }

                return len;
}
#line 149 "./src/util/binder.lzz"
Binder::Result Binder::BindArgs (v8::FunctionCallbackInfo <v8 :: Value> const & info, int argc, Statement * stmt)
#line 149 "./src/util/binder.lzz"
                                                                        {
                v8 :: Isolate * isolate = info . GetIsolate ( ) ;
                int count = 0;
                bool bound_object = false;

                for (int i = 0; i < argc; ++i) {
                        v8::Local<v8::Value> arg = info[i];

                        if (arg->IsArray()) {
                                count += BindArray(isolate, arg.As<v8::Array>());
                                if (!success) break;
                                continue;
                        }

                        if (arg->IsObject() && !node::Buffer::HasInstance(arg)) {
                                v8::Local<v8::Object> obj = arg.As<v8::Object>();
                                if (IsPlainObject(isolate, obj)) {
                                        if (bound_object) {
                                                Fail(ThrowTypeError, "You cannot specify named parameters in two different objects");
                                                break;
                                        }
                                        bound_object = true;

                                        count += BindObject(isolate, obj, stmt);
                                        if (!success) break;
                                        continue;
                                } else if (stmt->GetBindMap(isolate)->GetSize()) {
                                        Fail(ThrowTypeError, "Named parameters can only be passed within plain objects");
                                        break;
                                }
                        }

                        BindValue(isolate, arg, NextAnonIndex());
                        if (!success) break;
                        count += 1;
                }

                return { count, bound_object };
}
#line 35 "./src/better_sqlite3.lzz"
void Addon::JS_setErrorConstructor (v8::FunctionCallbackInfo <v8 :: Value> const & info)
#line 35 "./src/better_sqlite3.lzz"
                                            {
                if ( info . Length ( ) <= ( 0 ) || ! info [ 0 ] -> IsFunction ( ) ) return ThrowTypeError ( "Expected " "first" " argument to be " "a function" ) ; v8 :: Local < v8 :: Function > SqliteError = ( info [ 0 ] . As < v8 :: Function > ( ) ) ;
                static_cast < Addon * > ( info . Data ( ) . As < v8 :: External > ( ) -> Value ( ) ) ->SqliteError.Reset( info . GetIsolate ( ) , SqliteError);
}
#line 40 "./src/better_sqlite3.lzz"
void Addon::Cleanup (void * ptr)
#line 40 "./src/better_sqlite3.lzz"
                                       {
                Addon* addon = static_cast<Addon*>(ptr);
                for (Database* db : addon->dbs) db->CloseHandles();
                addon->dbs.clear();
                delete addon;
}
#line 47 "./src/better_sqlite3.lzz"
Addon::Addon (v8::Isolate * isolate)
#line 47 "./src/better_sqlite3.lzz"
  : privileged_info (NULL), next_id (0), cs (isolate)
#line 50 "./src/better_sqlite3.lzz"
                            {}
#undef LZZ_INLINE
