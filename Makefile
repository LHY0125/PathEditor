CC = gcc
WINDRES = windres

# Paths - specific to user environment
IUP_DIR = libs/iup-3.31_Win64_dllw6_lib
INCLUDE_DIR = $(IUP_DIR)/include
LIB_DIR = $(IUP_DIR)
LOCAL_INCLUDE_DIR = include

# Output Directories
OBJ_DIR = obj
BIN_DIR = bin

# Flags
# -mwindows: Create GUI app (no console)
# -DUNICODE -D_UNICODE: Use Wide Character API
CFLAGS = -Wall -O2 -I$(INCLUDE_DIR) -I$(LOCAL_INCLUDE_DIR) -D_WIN32 -DUNICODE -D_UNICODE -fexec-charset=UTF-8
LDFLAGS = -L$(LIB_DIR) -liup -liupcd -lgdi32 -lcomdlg32 -lcomctl32 -luuid -lole32 -ladvapi32 -mwindows

# Source
SRC = src/main.c src/utils.c src/registry.c src/ui.c src/ui_utils.c src/cb_edit.c src/cb_file.c src/cb_main.c
RES = ico/resources.rc
OBJ = $(OBJ_DIR)/main.o $(OBJ_DIR)/utils.o $(OBJ_DIR)/registry.o $(OBJ_DIR)/ui.o $(OBJ_DIR)/ui_utils.o $(OBJ_DIR)/cb_edit.o $(OBJ_DIR)/cb_file.o $(OBJ_DIR)/cb_main.o $(OBJ_DIR)/resources.o
EXE = $(BIN_DIR)/PathEditor.exe

all: $(BIN_DIR) $(OBJ_DIR) $(EXE)

$(BIN_DIR):
	if not exist $(BIN_DIR) mkdir $(BIN_DIR)

$(OBJ_DIR):
	if not exist $(OBJ_DIR) mkdir $(OBJ_DIR)

$(EXE): $(OBJ)
	$(CC) -o $@ $^ $(LDFLAGS)

$(OBJ_DIR)/main.o: src/main.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/utils.o: src/utils.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/registry.o: src/registry.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/ui.o: src/ui.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/ui_utils.o: src/ui_utils.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/cb_edit.o: src/cb_edit.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/cb_file.o: src/cb_file.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/cb_main.o: src/cb_main.c
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJ_DIR)/resources.o: ico/resources.rc
	$(WINDRES) -i $< -o $@

clean:
	if exist $(OBJ_DIR)\*.o del /Q $(OBJ_DIR)\*.o
	if exist $(BIN_DIR)\*.exe del /Q $(BIN_DIR)\*.exe
