#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

extern "C" {

void readdir(sqlite3 *handle, const char *path_ptr);

} // extern "C"
