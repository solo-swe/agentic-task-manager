"""atm-lib: all of atm's business logic (docs/layered-architecture.md §5). Its functions take a Project and plain
values and return results; they never parse arguments, print or exit, and reach data only through the store's
interface. Function names are unique across atm (`show_tasks`, not `tasks.show`): the build puts every module in one
namespace."""
