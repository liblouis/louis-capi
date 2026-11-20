/* Checks that louis.h and the library work from C. Run from the repository root:
 *   cargo build && cc -Wall -Wextra -Werror -std=c99 -I include tests/smoke.c \
 *     -L target/debug -llouis_capi -o target/smoke && LD_LIBRARY_PATH=target/debug target/smoke
 */
#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "louis.h"

int main(void) {
  const char *tables[] = {"test.ctb"};
  const char *search_path[] = {"tests"};
  LouisTranslator *translator;
  LouisStatus status =
      louis_translator_new(tables, 1, search_path, 1, LOUIS_FORWARD, &translator);
  assert(status == LOUIS_OK);

  const char *input = "ab ba";
  size_t cursor = 4, len, cell_cursor;
  LouisResult *result;
  status = louis_translate(translator, input, strlen(input), 0, &cursor, &result);
  assert(status == LOUIS_OK);
  const char *output = louis_result_output(result, &len);
  assert(strcmp(output, "⠹⠀⠃⠁") == 0 && len == strlen(output));
  const size_t *positions = louis_result_input_positions(result, &len);
  assert(len == 4 && positions[0] == 0 && positions[1] == 2);
  assert(louis_result_cursor(result, &cell_cursor) && cell_cursor == 3);
  louis_result_free(result);
  louis_translator_free(translator);

  const char *missing[] = {"missing.ctb"};
  status = louis_translator_new(missing, 1, search_path, 1, LOUIS_FORWARD, &translator);
  assert(status == LOUIS_TABLE_ERROR && translator == NULL);
  assert(strstr(louis_last_error(), "missing.ctb"));

  printf("louis-rs %s: ok\n", louis_version());
  return 0;
}
