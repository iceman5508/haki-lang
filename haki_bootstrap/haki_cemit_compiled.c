/* haki_cemit_compiled.c — functions only (runtime stripped) */
/* ── Functions ── */
compiler__Lexer* compiler__lexerNew(const char* src) {
    return ({ compiler__Lexer* __c_compiler__Lexer = (compiler__Lexer*)malloc(sizeof(compiler__Lexer)); __c_compiler__Lexer->src = src; __c_compiler__Lexer->pos = ((int64_t)0LL); __c_compiler__Lexer->len = haki_string_length(src); __c_compiler__Lexer; });
}

const char* compiler__charAt(compiler__Lexer* l, int64_t i) {
    if ((i >= l->len)) {
        return "";
    }
    return haki_string_substring(l->src, i, (i + ((int64_t)1LL)));
}

int8_t compiler__isDigit(const char* ch) {
    if ((strcmp(ch, "0") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "1") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "2") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "3") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "4") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "5") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "6") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "7") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "8") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "9") == 0)) {
        return 1;
    }
    return 0;
}

int8_t compiler__isAlpha(const char* ch) {
    if ((strcmp(ch, "_") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "a") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "A") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "b") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "B") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "c") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "C") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "d") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "D") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "e") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "E") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "f") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "F") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "g") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "G") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "h") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "H") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "i") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "I") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "j") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "J") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "k") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "K") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "l") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "L") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "m") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "M") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "n") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "N") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "o") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "O") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "p") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "P") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "q") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "Q") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "r") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "R") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "s") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "S") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "t") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "T") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "u") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "U") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "v") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "V") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "w") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "W") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "x") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "X") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "y") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "Y") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "z") == 0)) {
        return 1;
    }
    if ((strcmp(ch, "Z") == 0)) {
        return 1;
    }
    return 0;
}

int8_t compiler__isAlphaNum(const char* ch) {
    return (compiler__isAlpha(ch) || compiler__isDigit(ch));
}

int8_t compiler__isWhitespace(const char* ch) {
    return ((((strcmp(ch, " ") == 0) || (strcmp(ch, "\n") == 0)) || (strcmp(ch, "\t") == 0)) || (strcmp(ch, "\r") == 0));
}

void* compiler__identToKeyword(const char* s) {
    if ((strcmp(s, "fn") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "let") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 5LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "const") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 6LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "return") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 7LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "if") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 8LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "else") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 9LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "while") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 10LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "for") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 11LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "in") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 12LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "match") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 13LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "yield") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 14LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "struct") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 15LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "class") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 16LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "enum") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 17LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "protocol") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 18LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "impl") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 19LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "import") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 20LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "as") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 21LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "weak") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 22LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "async") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 23LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "await") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 24LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "defer") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 25LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "try") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 26LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "true") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 27LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "false") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 28LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    if ((strcmp(s, "null") == 0)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 29LL; ((void**)__ev)[1] = NULL; __ev; }));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 59LL; ((void**)__ev)[1] = NULL; __ev; }));
    { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 0; __ret->f1 = __f1; }
    return __ret;
}

int64_t compiler__digitVal(const char* ch) {
    if ((strcmp(ch, "0") == 0)) {
        return ((int64_t)0LL);
    }
    if ((strcmp(ch, "1") == 0)) {
        return ((int64_t)1LL);
    }
    if ((strcmp(ch, "2") == 0)) {
        return ((int64_t)2LL);
    }
    if ((strcmp(ch, "3") == 0)) {
        return ((int64_t)3LL);
    }
    if ((strcmp(ch, "4") == 0)) {
        return ((int64_t)4LL);
    }
    if ((strcmp(ch, "5") == 0)) {
        return ((int64_t)5LL);
    }
    if ((strcmp(ch, "6") == 0)) {
        return ((int64_t)6LL);
    }
    if ((strcmp(ch, "7") == 0)) {
        return ((int64_t)7LL);
    }
    if ((strcmp(ch, "8") == 0)) {
        return ((int64_t)8LL);
    }
    if ((strcmp(ch, "9") == 0)) {
        return ((int64_t)9LL);
    }
    return ((int64_t)0LL);
}

void* compiler__tokenize(const char* src) {
    compiler__Lexer* l = compiler__lexerNew(src);
    void* tokens = haki_array_new(sizeof(void*));
    while ((l->pos < l->len)) {
        const char* ch = compiler__charAt(l, l->pos);
        int64_t lo = l->pos;
        if (compiler__isWhitespace(ch)) {
            (l->pos = (l->pos + ((int64_t)1LL)));
            continue;
        }
        if (((strcmp(ch, "/") == 0) && (strcmp(compiler__charAt(l, (l->pos + ((int64_t)1LL))), "/") == 0))) {
            while (((l->pos < l->len) && (!(strcmp(compiler__charAt(l, l->pos), "\n") == 0)))) {
                (l->pos = (l->pos + ((int64_t)1LL)));
            }
            continue;
        }
        if (compiler__isDigit(ch)) {
            int64_t n = ((int64_t)0LL);
            while (((l->pos < l->len) && compiler__isDigit(compiler__charAt(l, l->pos)))) {
                (n = ((n * ((int64_t)10LL)) + compiler__digitVal(compiler__charAt(l, l->pos))));
                (l->pos = (l->pos + ((int64_t)1LL)));
            }
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ int64_t* __pl = (int64_t*)malloc(sizeof(int64_t)); *__pl = n; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = __pl; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if (compiler__isAlpha(ch)) {
            int64_t start = l->pos;
            while (((l->pos < l->len) && compiler__isAlphaNum(compiler__charAt(l, l->pos)))) {
                (l->pos = (l->pos + ((int64_t)1LL)));
            }
            const char* word = haki_string_substring(src, start, l->pos);
            __Tuple2* __mb_8266 = (__Tuple2*)(compiler__identToKeyword(word));
            compiler__TokenKind* kw = (compiler__TokenKind*)__mb_8266->f0;
            int8_t isKw = *(int8_t*)__mb_8266->f1;
            if (isKw) {
                { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = kw; __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            }
            else {
                { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = word; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 3LL; ((void**)__ev)[1] = __pl; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            }
            continue;
        }
        if ((strcmp(ch, "\"") == 0)) {
            (l->pos = (l->pos + ((int64_t)1LL)));
            int64_t start = l->pos;
            const char* buf = "";
            while (((l->pos < l->len) && (!(strcmp(compiler__charAt(l, l->pos), "\"") == 0)))) {
                const char* c = compiler__charAt(l, l->pos);
                if (((strcmp(c, "\\") == 0) && ((l->pos + ((int64_t)1LL)) < l->len))) {
                    const char* esc = compiler__charAt(l, (l->pos + ((int64_t)1LL)));
                    if ((strcmp(esc, "n") == 0)) {
                        (buf = haki_string_concat(buf, "\n"));
                        (l->pos = (l->pos + ((int64_t)2LL)));
                        continue;
                    }
                    if ((strcmp(esc, "t") == 0)) {
                        (buf = haki_string_concat(buf, "\t"));
                        (l->pos = (l->pos + ((int64_t)2LL)));
                        continue;
                    }
                    if ((strcmp(esc, "\"") == 0)) {
                        (buf = haki_string_concat(buf, "\""));
                        (l->pos = (l->pos + ((int64_t)2LL)));
                        continue;
                    }
                    if ((strcmp(esc, "\\") == 0)) {
                        (buf = haki_string_concat(buf, "\\"));
                        (l->pos = (l->pos + ((int64_t)2LL)));
                        continue;
                    }
                }
                (buf = haki_string_concat(buf, c));
                (l->pos = (l->pos + ((int64_t)1LL)));
            }
            if ((l->pos < l->len)) {
                (l->pos = (l->pos + ((int64_t)1LL)));
            }
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = buf; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = __pl; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        const char* ch2 = compiler__charAt(l, (l->pos + ((int64_t)1LL)));
        if (((strcmp(ch, "-") == 0) && (strcmp(ch2, ">") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 40LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, "=") == 0) && (strcmp(ch2, ">") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 41LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, "=") == 0) && (strcmp(ch2, "=") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 46LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, "!") == 0) && (strcmp(ch2, "=") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 47LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, "<") == 0) && (strcmp(ch2, "=") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 49LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, ">") == 0) && (strcmp(ch2, "=") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 51LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, "&") == 0) && (strcmp(ch2, "&") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 57LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        if (((strcmp(ch, "|") == 0) && (strcmp(ch2, "|") == 0))) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 58LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = (lo + ((int64_t)2LL)); __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            (l->pos = (l->pos + ((int64_t)2LL)));
            continue;
        }
        (l->pos = (l->pos + ((int64_t)1LL)));
        if ((strcmp(ch, "(") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 30LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, ")") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 31LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "{") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 32LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "}") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 33LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "[") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 34LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "]") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 35LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, ",") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 36LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, ".") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 37LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, ":") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 38LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "?") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 42LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "_") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 43LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "!") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 44LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "=") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 45LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "<") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 48LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, ">") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 50LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "+") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 52LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "-") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 53LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "*") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 54LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "/") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 55LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        if ((strcmp(ch, "%") == 0)) {
            { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 56LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
            continue;
        }
        { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = haki_string_concat("unexpected char: ", ch); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 60LL; ((void**)__ev)[1] = __pl; __ev; }); __c_compiler__Token->lo = lo; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
    }
    { compiler__Token* __append_tmp = (({ compiler__Token* __c_compiler__Token = (compiler__Token*)malloc(sizeof(compiler__Token)); __c_compiler__Token->kind = ({ compiler__TokenKind* __ev = (compiler__TokenKind*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 59LL; ((void**)__ev)[1] = NULL; __ev; }); __c_compiler__Token->lo = l->pos; __c_compiler__Token->hi = l->pos; __c_compiler__Token; })); haki_array_append_val(tokens, &__append_tmp); };
    return tokens;
}

const char* compiler__showKind(compiler__TokenKind* k) {
    const char* s = ({ const char* __match_result;  void* __msc = (void*)k;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int64_t n = *(int64_t*)__mpayload; __match_result = haki_string_concat(haki_string_concat("Int(", haki_int_to_string(n)), ")"); } else if (__mtag == 2LL) { const char* s = *(const char**)__mpayload; __match_result = haki_string_concat(haki_string_concat("Str(", s), ")"); } else if (__mtag == 3LL) { const char* s = *(const char**)__mpayload; __match_result = haki_string_concat(haki_string_concat("Ident(", s), ")"); } else if (__mtag == 60LL) { const char* s = *(const char**)__mpayload; __match_result = haki_string_concat(haki_string_concat("Error(", s), ")"); } else if (__mtag == 4LL) { __match_result = "fn"; } else if (__mtag == 5LL) { __match_result = "let"; } else if (__mtag == 6LL) { __match_result = "const"; } else if (__mtag == 7LL) { __match_result = "return"; } else if (__mtag == 8LL) { __match_result = "if"; } else if (__mtag == 9LL) { __match_result = "else"; } else if (__mtag == 10LL) { __match_result = "while"; } else if (__mtag == 11LL) { __match_result = "for"; } else if (__mtag == 12LL) { __match_result = "in"; } else if (__mtag == 13LL) { __match_result = "match"; } else if (__mtag == 14LL) { __match_result = "yield"; } else if (__mtag == 15LL) { __match_result = "struct"; } else if (__mtag == 16LL) { __match_result = "class"; } else if (__mtag == 17LL) { __match_result = "enum"; } else if (__mtag == 18LL) { __match_result = "protocol"; } else if (__mtag == 19LL) { __match_result = "impl"; } else if (__mtag == 20LL) { __match_result = "import"; } else if (__mtag == 21LL) { __match_result = "as"; } else if (__mtag == 22LL) { __match_result = "weak"; } else if (__mtag == 23LL) { __match_result = "async"; } else if (__mtag == 24LL) { __match_result = "await"; } else if (__mtag == 25LL) { __match_result = "defer"; } else if (__mtag == 26LL) { __match_result = "try"; } else if (__mtag == 27LL) { __match_result = "true"; } else if (__mtag == 28LL) { __match_result = "false"; } else if (__mtag == 29LL) { __match_result = "null"; } else if (__mtag == 30LL) { __match_result = "("; } else if (__mtag == 31LL) { __match_result = ")"; } else if (__mtag == 32LL) { __match_result = "{"; } else if (__mtag == 33LL) { __match_result = "}"; } else if (__mtag == 34LL) { __match_result = "["; } else if (__mtag == 35LL) { __match_result = "]"; } else if (__mtag == 36LL) { __match_result = ","; } else if (__mtag == 37LL) { __match_result = "."; } else if (__mtag == 38LL) { __match_result = ":"; } else if (__mtag == 42LL) { __match_result = "?"; } else if (__mtag == 43LL) { __match_result = "_"; } else if (__mtag == 44LL) { __match_result = "!"; } else if (__mtag == 40LL) { __match_result = "->"; } else if (__mtag == 41LL) { __match_result = "=>"; } else if (__mtag == 45LL) { __match_result = "="; } else if (__mtag == 46LL) { __match_result = "=="; } else if (__mtag == 47LL) { __match_result = "!="; } else if (__mtag == 48LL) { __match_result = "<"; } else if (__mtag == 49LL) { __match_result = "<="; } else if (__mtag == 50LL) { __match_result = ">"; } else if (__mtag == 51LL) { __match_result = ">="; } else if (__mtag == 52LL) { __match_result = "+"; } else if (__mtag == 53LL) { __match_result = "-"; } else if (__mtag == 54LL) { __match_result = "*"; } else if (__mtag == 55LL) { __match_result = "/"; } else if (__mtag == 56LL) { __match_result = "%"; } else if (__mtag == 57LL) { __match_result = "&&"; } else if (__mtag == 58LL) { __match_result = "||"; } else if (__mtag == 59LL) { __match_result = "EOF"; } else if (__mtag == 39LL) { __match_result = ";"; } else if (__mtag == 1LL) { int64_t n = *(int64_t*)__mpayload; __match_result = haki_string_concat(haki_string_concat("Float(", haki_int_to_string(n)), ")"); } __match_result; });
    return s;
}

compiler__Parser* compiler__parserNew(void* tokens) {
    return ({ compiler__Parser* __c_compiler__Parser = (compiler__Parser*)malloc(sizeof(compiler__Parser)); __c_compiler__Parser->tokens = tokens; __c_compiler__Parser->pos = ((int64_t)0LL); __c_compiler__Parser; });
}

compiler__Token* compiler__peek(compiler__Parser* p) {
    if ((p->pos < haki_array_length(p->tokens))) {
        return (*(compiler__Token**)haki_array_get(p->tokens, p->pos));
    }
    return (*(compiler__Token**)haki_array_get(p->tokens, (haki_array_length(p->tokens) - ((int64_t)1LL))));
}

const char* compiler__peekKind(compiler__Parser* p) {
    return compiler__showKind(compiler__peek(p)->kind);
}

compiler__Token* compiler__advance(compiler__Parser* p) {
    compiler__Token* tok = compiler__peek(p);
    if ((p->pos < (haki_array_length(p->tokens) - ((int64_t)1LL)))) {
        (p->pos = (p->pos + ((int64_t)1LL)));
    }
    return tok;
}

int8_t compiler__eat(compiler__Parser* p, const char* kind) {
    if ((strcmp(compiler__peekKind(p), kind) == 0)) {
        (void)(compiler__advance(p));
        return 1;
    }
    return 0;
}

void* compiler__expect(compiler__Parser* p, const char* kind) {
    compiler__Token* tok = compiler__peek(p);
    if ((!(strcmp(compiler__peekKind(p), kind) == 0))) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(tok);
        __ret->f1 = (void*)(haki_error_new(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("expected ", kind), " but got "), compiler__peekKind(p)), " at pos "), haki_int_to_string(p->pos))));
        return __ret;
    }
    (void)(compiler__advance(p));
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(tok);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseSimpleTyStr(compiler__Parser* p) {
    const char* kind = compiler__peekKind(p);
    if ((strcmp(kind, "(") == 0)) {
        (void)(compiler__advance(p));
        void* parts = haki_array_new(sizeof(void*));
        while (((!(strcmp(compiler__peekKind(p), ")") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
            __Tuple2* __mb_18904 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
            const char* part = (const char*)__mb_18904->f0;
            void* pe = (void*)__mb_18904->f1;
            if ((pe != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)("()");
                __ret->f1 = (void*)(pe);
                return __ret;
            }
            haki_array_append_val(parts, &(part));
            (void)(compiler__eat(p, ","));
        }
        __Tuple2* __mb_19070 = (__Tuple2*)(compiler__expect(p, ")"));
        void* rpe = (void*)__mb_19070->f1;
        if ((rpe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)("()");
            __ret->f1 = (void*)(rpe);
            return __ret;
        }
        const char* result = "(";
        int64_t i = ((int64_t)0LL);
        while ((i < haki_array_length(parts))) {
            if ((i > ((int64_t)0LL))) {
                (result = haki_string_concat(result, ", "));
            }
            (result = haki_string_concat(result, (*(const char**)haki_array_get(parts, i))));
            (i = (i + ((int64_t)1LL)));
        }
        (result = haki_string_concat(result, ")"));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(result);
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "fn") == 0)) {
        (void)(compiler__advance(p));
        const char* fnTy = "fn";
        if ((strcmp(compiler__peekKind(p), "(") == 0)) {
            (void)(compiler__advance(p));
            (fnTy = haki_string_concat(fnTy, "("));
            int8_t first = 1;
            while (((!(strcmp(compiler__peekKind(p), ")") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
                __Tuple2* __mb_19783 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
                const char* pt = (const char*)__mb_19783->f0;
                void* pte = (void*)__mb_19783->f1;
                if ((pte != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(haki_string_concat(fnTy, ")"));
                    __ret->f1 = (void*)(pte);
                    return __ret;
                }
                if ((!first)) {
                    (fnTy = haki_string_concat(fnTy, ", "));
                }
                (fnTy = haki_string_concat(fnTy, pt));
                (first = 0);
                (void)(compiler__eat(p, ","));
            }
            __Tuple2* __mb_20053 = (__Tuple2*)(compiler__expect(p, ")"));
            void* rpe = (void*)__mb_20053->f1;
            if ((rpe != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(fnTy);
                __ret->f1 = (void*)(rpe);
                return __ret;
            }
            (fnTy = haki_string_concat(fnTy, ")"));
        }
        if ((strcmp(compiler__peekKind(p), "->") == 0)) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_20249 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
            const char* rt = (const char*)__mb_20249->f0;
            void* rte = (void*)__mb_20249->f1;
            if ((rte != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(fnTy);
                __ret->f1 = (void*)(rte);
                return __ret;
            }
            (fnTy = haki_string_concat(haki_string_concat(fnTy, " -> "), rt));
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(fnTy);
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if (((haki_string_length(kind) > ((int64_t)6LL)) && (strcmp(haki_string_substring(kind, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
        const char* name = haki_string_substring(kind, ((int64_t)6LL), (haki_string_length(kind) - ((int64_t)1LL)));
        (void)(compiler__advance(p));
        const char* baseName = name;
        if ((strcmp(compiler__peekKind(p), ".") == 0)) {
            (void)(compiler__advance(p));
            const char* nextK = compiler__peekKind(p);
            if (((haki_string_length(nextK) > ((int64_t)6LL)) && (strcmp(haki_string_substring(nextK, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                const char* tyName = haki_string_substring(nextK, ((int64_t)6LL), (haki_string_length(nextK) - ((int64_t)1LL)));
                (void)(compiler__advance(p));
                (baseName = haki_string_concat(haki_string_concat(name, "."), tyName));
            }
        }
        if ((strcmp(compiler__peekKind(p), "<") == 0)) {
            (void)(compiler__advance(p));
            const char* argStr = haki_string_concat(baseName, "<");
            int8_t firstArg = 1;
            while (((!(strcmp(compiler__peekKind(p), ">") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
                __Tuple2* __mb_21382 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
                const char* inner = (const char*)__mb_21382->f0;
                void* ie = (void*)__mb_21382->f1;
                if ((ie != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(haki_string_concat(argStr, ">"));
                    __ret->f1 = (void*)(ie);
                    return __ret;
                }
                if ((!firstArg)) {
                    (argStr = haki_string_concat(argStr, ", "));
                }
                (argStr = haki_string_concat(argStr, inner));
                (firstArg = 0);
                (void)(compiler__eat(p, ","));
            }
            __Tuple2* __mb_21671 = (__Tuple2*)(compiler__expect(p, ">"));
            void* ge = (void*)__mb_21671->f1;
            if ((ge != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(haki_string_concat(argStr, ">"));
                __ret->f1 = (void*)(ge);
                return __ret;
            }
            (argStr = haki_string_concat(argStr, ">"));
            if ((strcmp(compiler__peekKind(p), "?") == 0)) {
                (void)(compiler__advance(p));
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(haki_string_concat(argStr, "?"));
                __ret->f1 = (void*)(NULL);
                return __ret;
            }
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(argStr);
            __ret->f1 = (void*)(NULL);
            return __ret;
        }
        if ((strcmp(compiler__peekKind(p), "?") == 0)) {
            (void)(compiler__advance(p));
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(haki_string_concat(baseName, "?"));
            __ret->f1 = (void*)(NULL);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(baseName);
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((((((strcmp(kind, "int") == 0) || (strcmp(kind, "string") == 0)) || (strcmp(kind, "bool") == 0)) || (strcmp(kind, "float") == 0)) || (strcmp(kind, "void") == 0))) {
        (void)(compiler__advance(p));
        if ((strcmp(compiler__peekKind(p), "?") == 0)) {
            (void)(compiler__advance(p));
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(haki_string_concat(kind, "?"));
            __ret->f1 = (void*)(NULL);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(kind);
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)("");
    __ret->f1 = (void*)(haki_error_new(haki_string_concat("expected type, got: ", kind)));
    return __ret;
}

void* compiler__parseExpr(compiler__Parser* p) {
    return compiler__parseOr(p);
}

void* compiler__parseOr(compiler__Parser* p) {
    __Tuple2* __mb_22785 = (__Tuple2*)(compiler__parseAnd(p));
    compiler__Expr* left = (compiler__Expr*)__mb_22785->f0;
    void* le = (void*)__mb_22785->f1;
    if ((le != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(left);
        __ret->f1 = (void*)(le);
        return __ret;
    }
    compiler__Expr* result = left;
    while ((strcmp(compiler__peekKind(p), "||") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_22943 = (__Tuple2*)(compiler__parseAnd(p));
        compiler__Expr* right = (compiler__Expr*)__mb_22943->f0;
        void* re = (void*)__mb_22943->f1;
        if ((re != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(result);
            __ret->f1 = (void*)(re);
            return __ret;
        }
        (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "||"; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = right; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseAnd(compiler__Parser* p) {
    __Tuple2* __mb_23143 = (__Tuple2*)(compiler__parseEquality(p));
    compiler__Expr* left = (compiler__Expr*)__mb_23143->f0;
    void* le = (void*)__mb_23143->f1;
    if ((le != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(left);
        __ret->f1 = (void*)(le);
        return __ret;
    }
    compiler__Expr* result = left;
    while ((strcmp(compiler__peekKind(p), "&&") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_23306 = (__Tuple2*)(compiler__parseEquality(p));
        compiler__Expr* right = (compiler__Expr*)__mb_23306->f0;
        void* re = (void*)__mb_23306->f1;
        if ((re != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(result);
            __ret->f1 = (void*)(re);
            return __ret;
        }
        (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "&&"; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = right; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseEquality(compiler__Parser* p) {
    __Tuple2* __mb_23516 = (__Tuple2*)(compiler__parseComparison(p));
    compiler__Expr* left = (compiler__Expr*)__mb_23516->f0;
    void* le = (void*)__mb_23516->f1;
    if ((le != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(left);
        __ret->f1 = (void*)(le);
        return __ret;
    }
    compiler__Expr* result = left;
    int8_t going = 1;
    while (going) {
        const char* k = compiler__peekKind(p);
        if (((strcmp(k, "==") == 0) || (strcmp(k, "!=") == 0))) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_23762 = (__Tuple2*)(compiler__parseComparison(p));
            compiler__Expr* right = (compiler__Expr*)__mb_23762->f0;
            void* re = (void*)__mb_23762->f1;
            if ((re != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(result);
                __ret->f1 = (void*)(re);
                return __ret;
            }
            (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = k; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = right; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        }
        else {
            (going = 0);
        }
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseComparison(compiler__Parser* p) {
    __Tuple2* __mb_24034 = (__Tuple2*)(compiler__parseAddSub(p));
    compiler__Expr* left = (compiler__Expr*)__mb_24034->f0;
    void* le = (void*)__mb_24034->f1;
    if ((le != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(left);
        __ret->f1 = (void*)(le);
        return __ret;
    }
    compiler__Expr* result = left;
    int8_t going = 1;
    while (going) {
        const char* k = compiler__peekKind(p);
        if (((((strcmp(k, "<") == 0) || (strcmp(k, "<=") == 0)) || (strcmp(k, ">") == 0)) || (strcmp(k, ">=") == 0))) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_24300 = (__Tuple2*)(compiler__parseAddSub(p));
            compiler__Expr* right = (compiler__Expr*)__mb_24300->f0;
            void* re = (void*)__mb_24300->f1;
            if ((re != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(result);
                __ret->f1 = (void*)(re);
                return __ret;
            }
            (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = k; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = right; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        }
        else {
            (going = 0);
        }
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseAddSub(compiler__Parser* p) {
    __Tuple2* __mb_24564 = (__Tuple2*)(compiler__parseMulDiv(p));
    compiler__Expr* left = (compiler__Expr*)__mb_24564->f0;
    void* le = (void*)__mb_24564->f1;
    if ((le != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(left);
        __ret->f1 = (void*)(le);
        return __ret;
    }
    compiler__Expr* result = left;
    int8_t going = 1;
    while (going) {
        const char* k = compiler__peekKind(p);
        if (((strcmp(k, "+") == 0) || (strcmp(k, "-") == 0))) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_24804 = (__Tuple2*)(compiler__parseMulDiv(p));
            compiler__Expr* right = (compiler__Expr*)__mb_24804->f0;
            void* re = (void*)__mb_24804->f1;
            if ((re != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(result);
                __ret->f1 = (void*)(re);
                return __ret;
            }
            (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = k; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = right; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        }
        else {
            (going = 0);
        }
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseMulDiv(compiler__Parser* p) {
    __Tuple2* __mb_25068 = (__Tuple2*)(compiler__parseUnary(p));
    compiler__Expr* left = (compiler__Expr*)__mb_25068->f0;
    void* le = (void*)__mb_25068->f1;
    if ((le != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(left);
        __ret->f1 = (void*)(le);
        return __ret;
    }
    compiler__Expr* result = left;
    int8_t going = 1;
    while (going) {
        const char* k = compiler__peekKind(p);
        if ((((strcmp(k, "*") == 0) || (strcmp(k, "/") == 0)) || (strcmp(k, "%") == 0))) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_25319 = (__Tuple2*)(compiler__parseUnary(p));
            compiler__Expr* right = (compiler__Expr*)__mb_25319->f0;
            void* re = (void*)__mb_25319->f1;
            if ((re != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(result);
                __ret->f1 = (void*)(re);
                return __ret;
            }
            (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = k; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = right; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        }
        else {
            (going = 0);
        }
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseUnary(compiler__Parser* p) {
    const char* k = compiler__peekKind(p);
    if (((strcmp(k, "-") == 0) || (strcmp(k, "!") == 0))) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_25670 = (__Tuple2*)(compiler__parsePostfix(p));
        compiler__Expr* operand = (compiler__Expr*)__mb_25670->f0;
        void* oe = (void*)__mb_25670->f1;
        if ((oe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(operand);
            __ret->f1 = (void*)(oe);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 5LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = k; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = operand; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    return compiler__parsePostfix(p);
}

void* compiler__parsePostfix(compiler__Parser* p) {
    __Tuple2* __mb_25878 = (__Tuple2*)(compiler__parsePrimary(p));
    compiler__Expr* base = (compiler__Expr*)__mb_25878->f0;
    void* be = (void*)__mb_25878->f1;
    if ((be != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(base);
        __ret->f1 = (void*)(be);
        return __ret;
    }
    compiler__Expr* result = base;
    int8_t going = 1;
    while (going) {
        const char* k = compiler__peekKind(p);
        if ((strcmp(k, ".") == 0)) {
            (void)(compiler__advance(p));
            const char* fieldK = compiler__peekKind(p);
            if (((haki_string_length(fieldK) > ((int64_t)6LL)) && (strcmp(haki_string_substring(fieldK, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                const char* field = haki_string_substring(fieldK, ((int64_t)6LL), (haki_string_length(fieldK) - ((int64_t)1LL)));
                (void)(compiler__advance(p));
                if ((strcmp(compiler__peekKind(p), "(") == 0)) {
                    (void)(compiler__advance(p));
                    __Tuple2* __mb_26430 = (__Tuple2*)(compiler__parseArgList(p));
                    void* args = (void*)__mb_26430->f0;
                    void* ae = (void*)__mb_26430->f1;
                    if ((ae != NULL)) {
                        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                        __ret->f0 = (void*)(result);
                        __ret->f1 = (void*)(ae);
                        return __ret;
                    }
                    __Tuple2* __mb_26539 = (__Tuple2*)(compiler__expect(p, ")"));
                    void* pe = (void*)__mb_26539->f1;
                    if ((pe != NULL)) {
                        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                        __ret->f0 = (void*)(result);
                        __ret->f1 = (void*)(pe);
                        return __ret;
                    }
                    (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 9LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = field; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = args; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                }
                else {
                    (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 8LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = field; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                }
            }
            else {
                (going = 0);
            }
        }
        else {
            if ((strcmp(k, "[") == 0)) {
                (void)(compiler__advance(p));
                __Tuple2* __mb_26919 = (__Tuple2*)(compiler__parseExpr(p));
                compiler__Expr* idx = (compiler__Expr*)__mb_26919->f0;
                void* ie = (void*)__mb_26919->f1;
                if ((ie != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(result);
                    __ret->f1 = (void*)(ie);
                    return __ret;
                }
                __Tuple2* __mb_27008 = (__Tuple2*)(compiler__expect(p, "]"));
                void* be2 = (void*)__mb_27008->f1;
                if ((be2 != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(result);
                    __ret->f1 = (void*)(be2);
                    return __ret;
                }
                (result = ({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 10LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = result; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = idx; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            }
            else {
                (going = 0);
            }
        }
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(result);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseArgList(compiler__Parser* p) {
    void* args = haki_array_new(sizeof(void*));
    while (((!(strcmp(compiler__peekKind(p), ")") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
        __Tuple2* __mb_27363 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* arg = (compiler__Expr*)__mb_27363->f0;
        void* ae = (void*)__mb_27363->f1;
        if ((ae != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(args);
            __ret->f1 = (void*)(ae);
            return __ret;
        }
        haki_array_append_val(args, &(arg));
        (void)(compiler__eat(p, ","));
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(args);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parsePrimary(compiler__Parser* p) {
    const char* kind = compiler__peekKind(p);
    if (((haki_string_length(kind) > ((int64_t)4LL)) && (strcmp(haki_string_substring(kind, ((int64_t)0LL), ((int64_t)4LL)), "Int(") == 0))) {
        const char* nStr = haki_string_substring(kind, ((int64_t)4LL), (haki_string_length(kind) - ((int64_t)1LL)));
        (void)(compiler__advance(p));
        int64_t n = ((int64_t)0LL);
        int64_t i = ((int64_t)0LL);
        int8_t neg = 0;
        if (((haki_string_length(nStr) > ((int64_t)0LL)) && (strcmp(haki_string_substring(nStr, ((int64_t)0LL), ((int64_t)1LL)), "-") == 0))) {
            (neg = 1);
            (i = ((int64_t)1LL));
        }
        while ((i < haki_string_length(nStr))) {
            const char* ch = haki_string_substring(nStr, i, (i + ((int64_t)1LL)));
            if ((strcmp(ch, "0") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)0LL)));
            }
            if ((strcmp(ch, "1") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)1LL)));
            }
            if ((strcmp(ch, "2") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)2LL)));
            }
            if ((strcmp(ch, "3") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)3LL)));
            }
            if ((strcmp(ch, "4") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)4LL)));
            }
            if ((strcmp(ch, "5") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)5LL)));
            }
            if ((strcmp(ch, "6") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)6LL)));
            }
            if ((strcmp(ch, "7") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)7LL)));
            }
            if ((strcmp(ch, "8") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)8LL)));
            }
            if ((strcmp(ch, "9") == 0)) {
                (n = ((n * ((int64_t)10LL)) + ((int64_t)9LL)));
            }
            (i = (i + ((int64_t)1LL)));
        }
        if (neg) {
            (n = (-n));
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ int64_t* __pl = (int64_t*)malloc(sizeof(int64_t)); *__pl = n; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if (((haki_string_length(kind) > ((int64_t)4LL)) && (strcmp(haki_string_substring(kind, ((int64_t)0LL), ((int64_t)4LL)), "Str(") == 0))) {
        const char* s = haki_string_substring(kind, ((int64_t)4LL), (haki_string_length(kind) - ((int64_t)1LL)));
        (void)(compiler__advance(p));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = s; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 3LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "true") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ int8_t* __pl = (int8_t*)malloc(sizeof(int8_t)); *__pl = 1; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "false") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ int8_t* __pl = (int8_t*)malloc(sizeof(int8_t)); *__pl = 0; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "null") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if (((haki_string_length(kind) > ((int64_t)6LL)) && (strcmp(haki_string_substring(kind, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
        const char* name = haki_string_substring(kind, ((int64_t)6LL), (haki_string_length(kind) - ((int64_t)1LL)));
        (void)(compiler__advance(p));
        if ((strcmp(compiler__peekKind(p), "(") == 0)) {
            (void)(compiler__advance(p));
            int8_t isNamed = 0;
            if (((haki_string_length(compiler__peekKind(p)) > ((int64_t)6LL)) && (strcmp(haki_string_substring(compiler__peekKind(p), ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                int64_t savedPos = p->pos;
                (void)(compiler__advance(p));
                if ((strcmp(compiler__peekKind(p), ":") == 0)) {
                    (isNamed = 1);
                }
                (p->pos = savedPos);
            }
            if (isNamed) {
                void* args = haki_array_new(sizeof(void*));
                while (((!(strcmp(compiler__peekKind(p), ")") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
                    if (((haki_string_length(compiler__peekKind(p)) > ((int64_t)6LL)) && (strcmp(haki_string_substring(compiler__peekKind(p), ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                        (void)(compiler__advance(p));
                    }
                    if ((strcmp(compiler__peekKind(p), ":") == 0)) {
                        (void)(compiler__advance(p));
                    }
                    __Tuple2* __mb_30507 = (__Tuple2*)(compiler__parseExpr(p));
                    compiler__Expr* arg = (compiler__Expr*)__mb_30507->f0;
                    void* ae = (void*)__mb_30507->f1;
                    if ((ae != NULL)) {
                        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = args; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                        __ret->f1 = (void*)(ae);
                        return __ret;
                    }
                    haki_array_append_val(args, &(arg));
                    (void)(compiler__eat(p, ","));
                }
                __Tuple2* __mb_30716 = (__Tuple2*)(compiler__expect(p, ")"));
                void* pe = (void*)__mb_30716->f1;
                if ((pe != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = args; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                    __ret->f1 = (void*)(pe);
                    return __ret;
                }
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = args; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(NULL);
                return __ret;
            }
            else {
                __Tuple2* __mb_30892 = (__Tuple2*)(compiler__parseArgList(p));
                void* args = (void*)__mb_30892->f0;
                void* ae = (void*)__mb_30892->f1;
                if ((ae != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = name; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = __pl; __ev; }));
                    __ret->f1 = (void*)(ae);
                    return __ret;
                }
                __Tuple2* __mb_30999 = (__Tuple2*)(compiler__expect(p, ")"));
                void* pe = (void*)__mb_30999->f1;
                if ((pe != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = name; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = __pl; __ev; }));
                    __ret->f1 = (void*)(pe);
                    return __ret;
                }
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = args; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(NULL);
                return __ret;
            }
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = name; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "match") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_31321 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* scrutinee = (compiler__Expr*)__mb_31321->f0;
        void* se = (void*)__mb_31321->f1;
        if ((se != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(scrutinee);
            __ret->f1 = (void*)(se);
            return __ret;
        }
        __Tuple2* __mb_31411 = (__Tuple2*)(compiler__expect(p, "{"));
        void* lbe = (void*)__mb_31411->f1;
        if ((lbe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(scrutinee);
            __ret->f1 = (void*)(lbe);
            return __ret;
        }
        void* arms = haki_array_new(sizeof(void*));
        while (((!(strcmp(compiler__peekKind(p), "}") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
            const char* pat = "_";
            const char* pk = compiler__peekKind(p);
            if ((strcmp(pk, "_") == 0)) {
                (void)(compiler__advance(p));
            }
            else {
                if (((haki_string_length(pk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(pk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                    (pat = haki_string_substring(pk, ((int64_t)6LL), (haki_string_length(pk) - ((int64_t)1LL))));
                    (void)(compiler__advance(p));
                }
            }
            void* bindings = haki_array_new(sizeof(void*));
            if ((strcmp(compiler__peekKind(p), "(") == 0)) {
                (void)(compiler__advance(p));
                while (((!(strcmp(compiler__peekKind(p), ")") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
                    const char* bk = compiler__peekKind(p);
                    if ((strcmp(bk, "_") == 0)) {
                        haki_array_append_val(bindings, &("_"));
                        (void)(compiler__advance(p));
                    }
                    else {
                        if (((haki_string_length(bk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(bk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                            { const char* __append_tmp = (haki_string_substring(bk, ((int64_t)6LL), (haki_string_length(bk) - ((int64_t)1LL)))); haki_array_append_val(bindings, &__append_tmp); };
                            (void)(compiler__advance(p));
                        }
                    }
                    (void)(compiler__eat(p, ","));
                }
                __Tuple2* __mb_32655 = (__Tuple2*)(compiler__expect(p, ")"));
                void* rpe = (void*)__mb_32655->f1;
                if ((rpe != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(scrutinee);
                    __ret->f1 = (void*)(rpe);
                    return __ret;
                }
            }
            __Tuple2* __mb_32792 = (__Tuple2*)(compiler__parseBlock(p));
            void* body = (void*)__mb_32792->f0;
            void* be = (void*)__mb_32792->f1;
            if ((be != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(scrutinee);
                __ret->f1 = (void*)(be);
                return __ret;
            }
            { compiler__MatchArm* __append_tmp = (({ compiler__MatchArm* __c_compiler__MatchArm = (compiler__MatchArm*)malloc(sizeof(compiler__MatchArm)); __c_compiler__MatchArm->pattern = pat; __c_compiler__MatchArm->bindings = bindings; __c_compiler__MatchArm->body = body; __c_compiler__MatchArm; })); haki_array_append_val(arms, &__append_tmp); };
        }
        __Tuple2* __mb_32972 = (__Tuple2*)(compiler__expect(p, "}"));
        void* rbe = (void*)__mb_32972->f1;
        if ((rbe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(scrutinee);
            __ret->f1 = (void*)(rbe);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 13LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = scrutinee; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = arms; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "{") == 0)) {
        __Tuple2* __mb_33167 = (__Tuple2*)(compiler__parseBlock(p));
        void* body = (void*)__mb_33167->f0;
        void* be = (void*)__mb_33167->f1;
        if ((be != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }));
            __ret->f1 = (void*)(be);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = body; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 14LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "if") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_33379 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* cond = (compiler__Expr*)__mb_33379->f0;
        void* ce = (void*)__mb_33379->f1;
        if ((ce != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(cond);
            __ret->f1 = (void*)(ce);
            return __ret;
        }
        __Tuple2* __mb_33459 = (__Tuple2*)(compiler__parseBlock(p));
        void* then = (void*)__mb_33459->f0;
        void* te = (void*)__mb_33459->f1;
        if ((te != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 12LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(te);
            return __ret;
        }
        void* els = haki_array_new(sizeof(void*));
        if ((strcmp(compiler__peekKind(p), "else") == 0)) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_33661 = (__Tuple2*)(compiler__parseBlock(p));
            void* elsBody = (void*)__mb_33661->f0;
            void* ee = (void*)__mb_33661->f1;
            if ((ee != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 12LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = els; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ee);
                return __ret;
            }
            (els = elsBody);
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 12LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = els; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "(") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_33925 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* e = (compiler__Expr*)__mb_33925->f0;
        void* ee = (void*)__mb_33925->f1;
        if ((ee != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(e);
            __ret->f1 = (void*)(ee);
            return __ret;
        }
        __Tuple2* __mb_33999 = (__Tuple2*)(compiler__expect(p, ")"));
        void* pe = (void*)__mb_33999->f1;
        if ((pe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(e);
            __ret->f1 = (void*)(pe);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(e);
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "[") == 0)) {
        (void)(compiler__advance(p));
        void* elems = haki_array_new(sizeof(void*));
        while (((!(strcmp(compiler__peekKind(p), "]") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
            __Tuple2* __mb_34275 = (__Tuple2*)(compiler__parseExpr(p));
            compiler__Expr* el = (compiler__Expr*)__mb_34275->f0;
            void* ee = (void*)__mb_34275->f1;
            if ((ee != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = elems; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 11LL; ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ee);
                return __ret;
            }
            haki_array_append_val(elems, &(el));
            (void)(compiler__eat(p, ","));
        }
        __Tuple2* __mb_34439 = (__Tuple2*)(compiler__expect(p, "]"));
        void* be = (void*)__mb_34439->f1;
        if ((be != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = elems; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 11LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(be);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = elems; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 11LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = "__error__"; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = __pl; __ev; }));
    __ret->f1 = (void*)(haki_error_new(haki_string_concat("unexpected token in expr: ", kind)));
    return __ret;
}

void* compiler__parseBlock(compiler__Parser* p) {
    __Tuple2* __mb_34905 = (__Tuple2*)(compiler__expect(p, "{"));
    void* lbe = (void*)__mb_34905->f1;
    if ((lbe != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(haki_array_new(sizeof(void*)));
        __ret->f1 = (void*)(lbe);
        return __ret;
    }
    void* stmts = haki_array_new(sizeof(void*));
    while (((!(strcmp(compiler__peekKind(p), "}") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
        __Tuple2* __mb_35068 = (__Tuple2*)(compiler__parseStmt(p));
        compiler__Stmt* stmt = (compiler__Stmt*)__mb_35068->f0;
        void* se = (void*)__mb_35068->f1;
        if ((se != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(stmts);
            __ret->f1 = (void*)(se);
            return __ret;
        }
        haki_array_append_val(stmts, &(stmt));
    }
    __Tuple2* __mb_35178 = (__Tuple2*)(compiler__expect(p, "}"));
    void* rbe = (void*)__mb_35178->f1;
    if ((rbe != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(stmts);
        __ret->f1 = (void*)(rbe);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(stmts);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseStmt(compiler__Parser* p) {
    const char* kind = compiler__peekKind(p);
    if ((strcmp(kind, "return") == 0)) {
        (void)(compiler__advance(p));
        void* vals = haki_array_new(sizeof(void*));
        if (((!(strcmp(compiler__peekKind(p), "}") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
            __Tuple2* __mb_35553 = (__Tuple2*)(compiler__parseExpr(p));
            compiler__Expr* e = (compiler__Expr*)__mb_35553->f0;
            void* ee = (void*)__mb_35553->f1;
            if ((ee != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = vals; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ee);
                return __ret;
            }
            haki_array_append_val(vals, &(e));
            while ((strcmp(compiler__peekKind(p), ",") == 0)) {
                (void)(compiler__advance(p));
                __Tuple2* __mb_35754 = (__Tuple2*)(compiler__parseExpr(p));
                compiler__Expr* e2 = (compiler__Expr*)__mb_35754->f0;
                void* e2e = (void*)__mb_35754->f1;
                if ((e2e != NULL)) {
                    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                    __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = vals; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
                    __ret->f1 = (void*)(e2e);
                    return __ret;
                }
                haki_array_append_val(vals, &(e2));
            }
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void** __pl = (void**)malloc(sizeof(void*)); *__pl = vals; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "yield") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_36004 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* e = (compiler__Expr*)__mb_36004->f0;
        void* ee = (void*)__mb_36004->f1;
        if ((ee != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(ee);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "defer") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_36178 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* e = (compiler__Expr*)__mb_36178->f0;
        void* ee = (void*)__mb_36178->f1;
        if ((ee != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 3LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(ee);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 3LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "continue") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__Stmt* __ev = (compiler__Stmt*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = NULL; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "break") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__Stmt* __ev = (compiler__Stmt*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 5LL; ((void**)__ev)[1] = NULL; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if (((strcmp(kind, "const") == 0) || (strcmp(kind, "let") == 0))) {
        int8_t isMut = (strcmp(kind, "let") == 0);
        (void)(compiler__advance(p));
        const char* firstK = compiler__peekKind(p);
        if (((haki_string_length(firstK) > ((int64_t)6LL)) && (strcmp(haki_string_substring(firstK, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
            const char* name = haki_string_substring(firstK, ((int64_t)6LL), (haki_string_length(firstK) - ((int64_t)1LL)));
            (void)(compiler__advance(p));
            if ((strcmp(compiler__peekKind(p), ",") == 0)) {
                while ((strcmp(compiler__peekKind(p), ",") == 0)) {
                    (void)(compiler__advance(p));
                    if ((strcmp(compiler__peekKind(p), "_") == 0)) {
                        (void)(compiler__advance(p));
                    }
                    else {
                        if (((haki_string_length(compiler__peekKind(p)) > ((int64_t)6LL)) && (strcmp(haki_string_substring(compiler__peekKind(p), ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                            (void)(compiler__advance(p));
                        }
                    }
                }
            }
            else {
                if ((strcmp(compiler__peekKind(p), ":") == 0)) {
                    (void)(compiler__advance(p));
                    __Tuple2* __mb_37563 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
                    void* te = (void*)__mb_37563->f1;
                    if ((te != NULL)) {
                        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                        __ret->f1 = (void*)(te);
                        return __ret;
                    }
                }
            }
            __Tuple2* __mb_37693 = (__Tuple2*)(compiler__expect(p, "="));
            void* eqe = (void*)__mb_37693->f1;
            if ((eqe != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(eqe);
                return __ret;
            }
            __Tuple2* __mb_37803 = (__Tuple2*)(compiler__parseExpr(p));
            compiler__Expr* init = (compiler__Expr*)__mb_37803->f0;
            void* ie = (void*)__mb_37803->f1;
            if ((ie != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ie);
                return __ret;
            }
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = name; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = init; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(NULL);
            return __ret;
        }
        if ((strcmp(compiler__peekKind(p), "_") == 0)) {
            (void)(compiler__advance(p));
            __Tuple2* __mb_38078 = (__Tuple2*)(compiler__expect(p, "="));
            void* eqe = (void*)__mb_38078->f1;
            if ((eqe != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "_"; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(eqe);
                return __ret;
            }
            __Tuple2* __mb_38187 = (__Tuple2*)(compiler__parseExpr(p));
            compiler__Expr* init = (compiler__Expr*)__mb_38187->f0;
            void* ie = (void*)__mb_38187->f1;
            if ((ie != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "_"; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ie);
                return __ret;
            }
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = isMut; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "_"; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = init; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(NULL);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { int8_t* __f = (int8_t*)malloc(sizeof(int8_t)); *__f = 0; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "__error__"; __pl[1] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(haki_error_new("expected name after let/const"));
        return __ret;
    }
    if ((strcmp(kind, "if") == 0)) {
        __Tuple2* __mb_38473 = (__Tuple2*)(compiler__parseIfStmt(p));
        compiler__Stmt* s = (compiler__Stmt*)__mb_38473->f0;
        void* se = (void*)__mb_38473->f1;
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(s);
        __ret->f1 = (void*)(se);
        return __ret;
    }
    if ((strcmp(kind, "while") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_38592 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* cond = (compiler__Expr*)__mb_38592->f0;
        void* ce = (void*)__mb_38592->f1;
        if ((ce != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 8LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(ce);
            return __ret;
        }
        __Tuple2* __mb_38684 = (__Tuple2*)(compiler__parseBlock(p));
        void* body = (void*)__mb_38684->f0;
        void* be = (void*)__mb_38684->f1;
        if ((be != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 8LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = body; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(be);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 8LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = body; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "for") == 0)) {
        (void)(compiler__advance(p));
        const char* varK = compiler__peekKind(p);
        const char* varName = "__it__";
        if (((haki_string_length(varK) > ((int64_t)6LL)) && (strcmp(haki_string_substring(varK, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
            (varName = haki_string_substring(varK, ((int64_t)6LL), (haki_string_length(varK) - ((int64_t)1LL))));
            (void)(compiler__advance(p));
        }
        if ((strcmp(compiler__peekKind(p), ",") == 0)) {
            (void)(compiler__advance(p));
            const char* vk = compiler__peekKind(p);
            if (((haki_string_length(vk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(vk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                (varName = haki_string_substring(vk, ((int64_t)6LL), (haki_string_length(vk) - ((int64_t)1LL))));
                (void)(compiler__advance(p));
            }
        }
        __Tuple2* __mb_39475 = (__Tuple2*)(compiler__expect(p, "in"));
        void* ine = (void*)__mb_39475->f1;
        if ((ine != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 9LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = varName; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(ine);
            return __ret;
        }
        __Tuple2* __mb_39578 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* iter = (compiler__Expr*)__mb_39578->f0;
        void* ie = (void*)__mb_39578->f1;
        if ((ie != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 9LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = varName; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = ({ compiler__Expr* __ev = (compiler__Expr*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; }); __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(ie);
            return __ret;
        }
        __Tuple2* __mb_39678 = (__Tuple2*)(compiler__parseBlock(p));
        void* body = (void*)__mb_39678->f0;
        void* be = (void*)__mb_39678->f1;
        if ((be != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 9LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = varName; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = iter; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = body; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(be);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 9LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = varName; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = iter; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = body; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    __Tuple2* __mb_39872 = (__Tuple2*)(compiler__parseExpr(p));
    compiler__Expr* e = (compiler__Expr*)__mb_39872->f0;
    void* ee = (void*)__mb_39872->f1;
    if ((ee != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(ee);
        return __ret;
    }
    if ((strcmp(compiler__peekKind(p), "=") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_40048 = (__Tuple2*)(compiler__parseExpr(p));
        compiler__Expr* rhs = (compiler__Expr*)__mb_40048->f0;
        void* re = (void*)__mb_40048->f1;
        if ((re != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(re);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 10LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = e; __pl[0] = __f; } { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = rhs; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ compiler__Expr** __pl = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__pl = e; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; ((void**)__ev)[1] = __pl; __ev; }));
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseIfStmt(compiler__Parser* p) {
    (void)(compiler__advance(p));
    __Tuple2* __mb_40290 = (__Tuple2*)(compiler__parseExpr(p));
    compiler__Expr* cond = (compiler__Expr*)__mb_40290->f0;
    void* ce = (void*)__mb_40290->f1;
    if ((ce != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(ce);
        return __ret;
    }
    __Tuple2* __mb_40375 = (__Tuple2*)(compiler__parseBlock(p));
    void* then = (void*)__mb_40375->f0;
    void* te = (void*)__mb_40375->f1;
    if ((te != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = haki_array_new(sizeof(void*)); __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(te);
        return __ret;
    }
    void* els = haki_array_new(sizeof(void*));
    if ((strcmp(compiler__peekKind(p), "else") == 0)) {
        (void)(compiler__advance(p));
        if ((strcmp(compiler__peekKind(p), "if") == 0)) {
            __Tuple2* __mb_40594 = (__Tuple2*)(compiler__parseIfStmt(p));
            compiler__Stmt* nested = (compiler__Stmt*)__mb_40594->f0;
            void* ne = (void*)__mb_40594->f1;
            if ((ne != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = els; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ne);
                return __ret;
            }
            haki_array_append_val(els, &(nested));
        }
        else {
            __Tuple2* __mb_40750 = (__Tuple2*)(compiler__parseBlock(p));
            void* elseBody = (void*)__mb_40750->f0;
            void* ee2 = (void*)__mb_40750->f1;
            if ((ee2 != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = els; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ee2);
                return __ret;
            }
            (els = elseBody);
        }
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; void** __pl = (void**)malloc(3 * sizeof(void*)); { compiler__Expr** __f = (compiler__Expr**)malloc(sizeof(compiler__Expr*)); *__f = cond; __pl[0] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = then; __pl[1] = __f; } { void** __f = (void**)malloc(sizeof(void*)); *__f = els; __pl[2] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseFn(compiler__Parser* p) {
    __Tuple2* __mb_41172 = (__Tuple2*)(compiler__expect(p, "fn"));
    void* fe = (void*)__mb_41172->f1;
    if ((fe != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = ""; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
        __ret->f1 = (void*)(fe);
        return __ret;
    }
    const char* nameK = compiler__peekKind(p);
    const char* name = "__anonymous__";
    if (((haki_string_length(nameK) > ((int64_t)6LL)) && (strcmp(haki_string_substring(nameK, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
        (name = haki_string_substring(nameK, ((int64_t)6LL), (haki_string_length(nameK) - ((int64_t)1LL))));
        (void)(compiler__advance(p));
    }
    __Tuple2* __mb_41521 = (__Tuple2*)(compiler__expect(p, "("));
    void* lpe = (void*)__mb_41521->f1;
    if ((lpe != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
        __ret->f1 = (void*)(lpe);
        return __ret;
    }
    void* params = haki_array_new(sizeof(void*));
    while (((!(strcmp(compiler__peekKind(p), ")") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
        const char* pk = compiler__peekKind(p);
        const char* pname = "__param__";
        if (((haki_string_length(pk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(pk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
            (pname = haki_string_substring(pk, ((int64_t)6LL), (haki_string_length(pk) - ((int64_t)1LL))));
            (void)(compiler__advance(p));
        }
        __Tuple2* __mb_41973 = (__Tuple2*)(compiler__expect(p, ":"));
        void* ce = (void*)__mb_41973->f1;
        if ((ce != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
            __ret->f1 = (void*)(ce);
            return __ret;
        }
        __Tuple2* __mb_42106 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
        const char* ty = (const char*)__mb_42106->f0;
        void* te = (void*)__mb_42106->f1;
        if ((te != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
            __ret->f1 = (void*)(te);
            return __ret;
        }
        { compiler__Param* __append_tmp = (({ compiler__Param* __c_compiler__Param = (compiler__Param*)malloc(sizeof(compiler__Param)); __c_compiler__Param->name = pname; __c_compiler__Param->ty = ty; __c_compiler__Param; })); haki_array_append_val(params, &__append_tmp); };
        (void)(compiler__eat(p, ","));
    }
    __Tuple2* __mb_42328 = (__Tuple2*)(compiler__expect(p, ")"));
    void* rpe = (void*)__mb_42328->f1;
    if ((rpe != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
        __ret->f1 = (void*)(rpe);
        return __ret;
    }
    const char* retTy = "void";
    if ((strcmp(compiler__peekKind(p), "->") == 0)) {
        (void)(compiler__advance(p));
        __Tuple2* __mb_42542 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
        const char* ty = (const char*)__mb_42542->f0;
        void* te = (void*)__mb_42542->f1;
        if ((te != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
            __ret->f1 = (void*)(te);
            return __ret;
        }
        (retTy = ty);
    }
    __Tuple2* __mb_42703 = (__Tuple2*)(compiler__parseBlock(p));
    void* body = (void*)__mb_42703->f0;
    void* be = (void*)__mb_42703->f1;
    if ((be != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = retTy; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }));
        __ret->f1 = (void*)(be);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = retTy; __c_compiler__FnDef->body = body; __c_compiler__FnDef; }));
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void* compiler__parseItem(compiler__Parser* p) {
    const char* kind = compiler__peekKind(p);
    if ((strcmp(kind, "import") == 0)) {
        (void)(compiler__advance(p));
        const char* pathK = compiler__peekKind(p);
        const char* path = "";
        if (((haki_string_length(pathK) > ((int64_t)4LL)) && (strcmp(haki_string_substring(pathK, ((int64_t)0LL), ((int64_t)4LL)), "Str(") == 0))) {
            (path = haki_string_substring(pathK, ((int64_t)4LL), (haki_string_length(pathK) - ((int64_t)1LL))));
            (void)(compiler__advance(p));
        }
        const char* alias = path;
        if ((strcmp(compiler__peekKind(p), "as") == 0)) {
            (void)(compiler__advance(p));
            const char* ak = compiler__peekKind(p);
            if (((haki_string_length(ak) > ((int64_t)6LL)) && (strcmp(haki_string_substring(ak, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                (alias = haki_string_substring(ak, ((int64_t)6LL), (haki_string_length(ak) - ((int64_t)1LL))));
                (void)(compiler__advance(p));
            }
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 2LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = path; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = alias; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "fn") == 0)) {
        __Tuple2* __mb_43673 = (__Tuple2*)(compiler__parseFn(p));
        compiler__FnDef* f = (compiler__FnDef*)__mb_43673->f0;
        void* fe = (void*)__mb_43673->f1;
        if ((fe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__FnDef** __pl = (compiler__FnDef**)malloc(sizeof(compiler__FnDef*)); *__pl = f; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(fe);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__FnDef** __pl = (compiler__FnDef**)malloc(sizeof(compiler__FnDef*)); *__pl = f; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "struct") == 0)) {
        (void)(compiler__advance(p));
        const char* nk = compiler__peekKind(p);
        const char* sname = "__struct__";
        if (((haki_string_length(nk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(nk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
            (sname = haki_string_substring(nk, ((int64_t)6LL), (haki_string_length(nk) - ((int64_t)1LL))));
            (void)(compiler__advance(p));
        }
        __Tuple2* __mb_44075 = (__Tuple2*)(compiler__expect(p, "{"));
        void* lbe = (void*)__mb_44075->f1;
        if ((lbe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = sname; __c_compiler__StructDef->fields = haki_array_new(sizeof(void*)); __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(lbe);
            return __ret;
        }
        void* fields = haki_array_new(sizeof(void*));
        while (((!(strcmp(compiler__peekKind(p), "}") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
            if (((strcmp(compiler__peekKind(p), "const") == 0) || (strcmp(compiler__peekKind(p), "let") == 0))) {
                (void)(compiler__advance(p));
            }
            const char* fk = compiler__peekKind(p);
            const char* fname = "__field__";
            if (((haki_string_length(fk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(fk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
                (fname = haki_string_substring(fk, ((int64_t)6LL), (haki_string_length(fk) - ((int64_t)1LL))));
                (void)(compiler__advance(p));
            }
            __Tuple2* __mb_44700 = (__Tuple2*)(compiler__expect(p, ":"));
            void* ce = (void*)__mb_44700->f1;
            if ((ce != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = sname; __c_compiler__StructDef->fields = fields; __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(ce);
                return __ret;
            }
            __Tuple2* __mb_44830 = (__Tuple2*)(compiler__parseSimpleTyStr(p));
            const char* ty = (const char*)__mb_44830->f0;
            void* te = (void*)__mb_44830->f1;
            if ((te != NULL)) {
                __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
                __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = sname; __c_compiler__StructDef->fields = fields; __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
                __ret->f1 = (void*)(te);
                return __ret;
            }
            { compiler__Param* __append_tmp = (({ compiler__Param* __c_compiler__Param = (compiler__Param*)malloc(sizeof(compiler__Param)); __c_compiler__Param->name = fname; __c_compiler__Param->ty = ty; __c_compiler__Param; })); haki_array_append_val(fields, &__append_tmp); };
        }
        __Tuple2* __mb_45026 = (__Tuple2*)(compiler__expect(p, "}"));
        void* rbe = (void*)__mb_45026->f1;
        if ((rbe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = sname; __c_compiler__StructDef->fields = fields; __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(rbe);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = sname; __c_compiler__StructDef->fields = fields; __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "enum") == 0)) {
        (void)(compiler__advance(p));
        const char* nk = compiler__peekKind(p);
        const char* ename = "__enum__";
        if (((haki_string_length(nk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(nk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
            (ename = haki_string_substring(nk, ((int64_t)6LL), (haki_string_length(nk) - ((int64_t)1LL))));
            (void)(compiler__advance(p));
        }
        if ((strcmp(compiler__peekKind(p), "<") == 0)) {
            while (((!(strcmp(compiler__peekKind(p), ">") == 0)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
                (void)(compiler__advance(p));
            }
            if ((strcmp(compiler__peekKind(p), ">") == 0)) {
                (void)(compiler__advance(p));
            }
        }
        __Tuple2* __mb_45926 = (__Tuple2*)(compiler__expect(p, "{"));
        void* lbe = (void*)__mb_45926->f1;
        if ((lbe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = ename; __c_compiler__StructDef->fields = haki_array_new(sizeof(void*)); __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(lbe);
            return __ret;
        }
        int64_t depth = ((int64_t)1LL);
        while (((depth > ((int64_t)0LL)) && (!(strcmp(compiler__peekKind(p), "EOF") == 0)))) {
            const char* tok = compiler__peekKind(p);
            if ((strcmp(tok, "{") == 0)) {
                (depth = (depth + ((int64_t)1LL)));
            }
            if ((strcmp(tok, "}") == 0)) {
                (depth = (depth - ((int64_t)1LL)));
            }
            if ((depth > ((int64_t)0LL))) {
                (void)(compiler__advance(p));
            }
        }
        __Tuple2* __mb_46375 = (__Tuple2*)(compiler__expect(p, "}"));
        void* rbe = (void*)__mb_46375->f1;
        if ((rbe != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = ename; __c_compiler__StructDef->fields = haki_array_new(sizeof(void*)); __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
            __ret->f1 = (void*)(rbe);
            return __ret;
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = ename; __c_compiler__StructDef->fields = haki_array_new(sizeof(void*)); __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if ((strcmp(kind, "class") == 0)) {
        (void)(compiler__advance(p));
        const char* nk = compiler__peekKind(p);
        const char* cname = "__class__";
        if (((haki_string_length(nk) > ((int64_t)6LL)) && (strcmp(haki_string_substring(nk, ((int64_t)0LL), ((int64_t)6LL)), "Ident(") == 0))) {
            (cname = haki_string_substring(nk, ((int64_t)6LL), (haki_string_length(nk) - ((int64_t)1LL))));
            (void)(compiler__advance(p));
        }
        if ((strcmp(compiler__peekKind(p), "extends") == 0)) {
            (void)(compiler__advance(p));
            (void)(compiler__advance(p));
        }
        int64_t depth = ((int64_t)0LL);
        while ((!(strcmp(compiler__peekKind(p), "EOF") == 0))) {
            const char* tok = compiler__peekKind(p);
            if ((strcmp(tok, "{") == 0)) {
                (depth = (depth + ((int64_t)1LL)));
            }
            if ((strcmp(tok, "}") == 0)) {
                (depth = (depth - ((int64_t)1LL)));
                if ((depth <= ((int64_t)0LL))) {
                    (void)(compiler__advance(p));
                    break;
                }
            }
            (void)(compiler__advance(p));
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ compiler__StructDef** __pl = (compiler__StructDef**)malloc(sizeof(compiler__StructDef*)); *__pl = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = cname; __c_compiler__StructDef->fields = haki_array_new(sizeof(void*)); __c_compiler__StructDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    if (((strcmp(kind, "protocol") == 0) || (strcmp(kind, "impl") == 0))) {
        (void)(compiler__advance(p));
        int64_t depth = ((int64_t)0LL);
        while ((!(strcmp(compiler__peekKind(p), "EOF") == 0))) {
            const char* tok = compiler__peekKind(p);
            if ((strcmp(tok, "{") == 0)) {
                (depth = (depth + ((int64_t)1LL)));
            }
            if ((strcmp(tok, "}") == 0)) {
                (depth = (depth - ((int64_t)1LL)));
                if ((depth <= ((int64_t)0LL))) {
                    (void)(compiler__advance(p));
                    break;
                }
            }
            (void)(compiler__advance(p));
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(({ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 2LL; void** __pl = (void**)malloc(2 * sizeof(void*)); { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "__skip__"; __pl[0] = __f; } { const char** __f = (const char**)malloc(sizeof(const char*)); *__f = "__skip__"; __pl[1] = __f; } ((void**)__ev)[1] = __pl; __ev; }));
        __ret->f1 = (void*)(NULL);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ compiler__FnDef** __pl = (compiler__FnDef**)malloc(sizeof(compiler__FnDef*)); *__pl = ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = "__error__"; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = "void"; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = __pl; __ev; }));
    __ret->f1 = (void*)(haki_error_new(haki_string_concat("unexpected top-level token: ", kind)));
    return __ret;
}

void* compiler__parse(const char* src) {
    void* tokens = compiler__tokenize(src);
    compiler__Parser* p = compiler__parserNew(tokens);
    void* items = haki_array_new(sizeof(void*));
    while ((!(strcmp(compiler__peekKind(p), "EOF") == 0))) {
        __Tuple2* __mb_48686 = (__Tuple2*)(compiler__parseItem(p));
        compiler__Item* item = (compiler__Item*)__mb_48686->f0;
        void* ie = (void*)__mb_48686->f1;
        if ((ie != NULL)) {
            __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
            __ret->f0 = (void*)(items);
            __ret->f1 = (void*)(ie);
            return __ret;
        }
        haki_array_append_val(items, &(item));
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(items);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

const char* compiler__showExpr(compiler__Expr* e) {
    const char* s = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int64_t n = *(int64_t*)__mpayload; __match_result = haki_string_concat(haki_string_concat("Int(", haki_int_to_string(n)), ")"); } else if (__mtag == 1LL) { int8_t b = *(int8_t*)__mpayload; __match_result = ((b) ? ("true") : ("false")); } else if (__mtag == 2LL) { __match_result = "null"; } else if (__mtag == 3LL) { const char* s = *(const char**)__mpayload; __match_result = haki_string_concat(haki_string_concat("\"", s), "\""); } else if (__mtag == 4LL) { const char* s = *(const char**)__mpayload; __match_result = s; } else if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* operand = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = haki_string_concat(op, compiler__showExpr(operand)); } else if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(", compiler__showExpr(l)), " "), op), " "), compiler__showExpr(r)), ")"); } else if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = haki_string_concat(n, "(...)"); } else if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = haki_string_concat(haki_string_concat(compiler__showExpr(recv), "."), f); } else if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* margs = *(void**)((void**)__mpayload)[2]; __match_result = haki_string_concat(haki_string_concat(haki_string_concat(compiler__showExpr(r), "."), m), "(...)"); } else if (__mtag == 10LL) { compiler__Expr* a = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* i = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = haki_string_concat(haki_string_concat(haki_string_concat(compiler__showExpr(a), "["), compiler__showExpr(i)), "]"); } else if (__mtag == 11LL) { void* aelems = *(void**)__mpayload; __match_result = "[...]"; } else if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = haki_string_concat(haki_string_concat("if ", compiler__showExpr(c)), " {...}"); } else if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = haki_string_concat(haki_string_concat("match ", compiler__showExpr(s)), " {...}"); } else if (__mtag == 14LL) { void* bstmts = *(void**)__mpayload; __match_result = "{...}"; } else if (__mtag == 15LL) { compiler__Expr* inner = *(compiler__Expr**)__mpayload; __match_result = haki_string_concat("async ", compiler__showExpr(inner)); } __match_result; });
    return s;
}

void compiler__main(void) {
    const char* src = "fn add(a: int, b: int) -> int { return a + b }";
    __Tuple2* __mb_50394 = (__Tuple2*)(compiler__parse(src));
    void* items = (void*)__mb_50394->f0;
    void* err = (void*)__mb_50394->f1;
    if ((err != NULL)) {
        haki_print(haki_string_concat("parse error: ", haki_error_message(err)));
        return;
    }
    haki_print(haki_string_concat(haki_string_concat("parsed ", haki_int_to_string(haki_array_length(items))), " item(s)"));
    { void* __arr_item = items;
        int64_t __len_item = haki_array_length(__arr_item);
        for (int64_t __i_item = 0; __i_item < __len_item; __i_item++) {
            compiler__Item* item = *(compiler__Item**)haki_array_get(__arr_item, __i_item);
            const char* desc = ({ const char* __match_result;  void* __msc = (void*)item;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = haki_string_concat("fn ", f->name); } else if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = haki_string_concat("struct ", s->name); } else if (__mtag == 2LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* a = *(const char**)((void**)__mpayload)[1]; __match_result = haki_string_concat("import ", p); } __match_result; });
            haki_print(desc);
        }
    }
}

void compiler__test_parse_fn(void) {
    __Tuple2* __mb_51070 = (__Tuple2*)(compiler__parse("fn add(a: int, b: int) -> int { return a + b }"));
    void* items = (void*)__mb_51070->f0;
    void* err = (void*)__mb_51070->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    if ((haki_array_length(items) != ((int64_t)1LL))) {
        haki_panic("expected 1 item");
    }
    const char* desc = ({ const char* __match_result;  void* __msc = (void*)(*(compiler__Item**)haki_array_get(items, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = haki_string_concat("fn:", f->name); } else if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = haki_string_concat("struct:", s->name); } else if (__mtag == 2LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* a = *(const char**)((void**)__mpayload)[1]; __match_result = haki_string_concat("import:", p); } __match_result; });
    if ((!(strcmp(desc, "fn:add") == 0))) {
        haki_panic(haki_string_concat("expected fn:add, got: ", desc));
    }
}

void compiler__test_parse_params(void) {
    __Tuple2* __mb_51536 = (__Tuple2*)(compiler__parse("fn greet(name: string, age: int) { return }"));
    void* items = (void*)__mb_51536->f0;
    void* err = (void*)__mb_51536->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    compiler__FnDef* f = ({ compiler__FnDef* __match_result;  void* __msc = (void*)(*(compiler__Item**)haki_array_get(items, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = f; } else { __match_result = ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = ""; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = ""; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }); } __match_result; });
    if ((haki_array_length(f->params) != ((int64_t)2LL))) {
        haki_panic(haki_string_concat("expected 2 params, got: ", haki_int_to_string(haki_array_length(f->params))));
    }
    if ((!(strcmp((*(compiler__Param**)haki_array_get(f->params, ((int64_t)0LL)))->name, "name") == 0))) {
        haki_panic(haki_string_concat("first param wrong: ", (*(compiler__Param**)haki_array_get(f->params, ((int64_t)0LL)))->name));
    }
    if ((!(strcmp((*(compiler__Param**)haki_array_get(f->params, ((int64_t)1LL)))->name, "age") == 0))) {
        haki_panic(haki_string_concat("second param wrong: ", (*(compiler__Param**)haki_array_get(f->params, ((int64_t)1LL)))->name));
    }
}

void compiler__test_parse_return_type(void) {
    __Tuple2* __mb_52084 = (__Tuple2*)(compiler__parse("fn id(x: int) -> int { return x }"));
    void* items = (void*)__mb_52084->f0;
    void* err = (void*)__mb_52084->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    compiler__FnDef* f = ({ compiler__FnDef* __match_result;  void* __msc = (void*)(*(compiler__Item**)haki_array_get(items, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = f; } else { __match_result = ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = ""; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = ""; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }); } __match_result; });
    if ((!(strcmp(f->retTy, "int") == 0))) {
        haki_panic(haki_string_concat("expected int return, got: ", f->retTy));
    }
}

void compiler__test_parse_struct(void) {
    __Tuple2* __mb_52418 = (__Tuple2*)(compiler__parse("struct Point { const x: int  const y: int }"));
    void* items = (void*)__mb_52418->f0;
    void* err = (void*)__mb_52418->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    compiler__StructDef* s = ({ compiler__StructDef* __match_result;  void* __msc = (void*)(*(compiler__Item**)haki_array_get(items, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = s; } else { __match_result = ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = ""; __c_compiler__StructDef->fields = haki_array_new(sizeof(void*)); __c_compiler__StructDef; }); } __match_result; });
    if ((!(strcmp(s->name, "Point") == 0))) {
        haki_panic(haki_string_concat("struct name wrong: ", s->name));
    }
    if ((haki_array_length(s->fields) != ((int64_t)2LL))) {
        haki_panic(haki_string_concat("expected 2 fields, got: ", haki_int_to_string(haki_array_length(s->fields))));
    }
}

void compiler__test_parse_expr_binary(void) {
    __Tuple2* __mb_52850 = (__Tuple2*)(compiler__parse("fn f() -> int { return 1 + 2 * 3 }"));
    void* items = (void*)__mb_52850->f0;
    void* err = (void*)__mb_52850->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    compiler__FnDef* f = ({ compiler__FnDef* __match_result;  void* __msc = (void*)(*(compiler__Item**)haki_array_get(items, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = f; } else { __match_result = ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = ""; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = ""; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }); } __match_result; });
    if ((haki_array_length(f->body) != ((int64_t)1LL))) {
        haki_panic("expected 1 stmt");
    }
    const char* retExpr = ({ const char* __match_result;  void* __msc = (void*)(*(compiler__Stmt**)haki_array_get(f->body, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 1LL) { void* vals = *(void**)__mpayload; __match_result = (((haki_array_length(vals) > ((int64_t)0LL))) ? (compiler__showExpr((*(compiler__Expr**)haki_array_get(vals, ((int64_t)0LL))))) : ("")); } else { __match_result = ""; } __match_result; });
    if ((!(strcmp(retExpr, "(Int(1) + (Int(2) * Int(3)))") == 0))) {
        haki_panic(haki_string_concat("wrong expr: ", retExpr));
    }
}

void compiler__test_parse_if(void) {
    __Tuple2* __mb_53455 = (__Tuple2*)(compiler__parse("fn f(x: int) -> int { if x > 0 { return x } return 0 }"));
    void* items = (void*)__mb_53455->f0;
    void* err = (void*)__mb_53455->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    compiler__FnDef* f = ({ compiler__FnDef* __match_result;  void* __msc = (void*)(*(compiler__Item**)haki_array_get(items, ((int64_t)0LL)));  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = f; } else { __match_result = ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = ""; __c_compiler__FnDef->params = haki_array_new(sizeof(void*)); __c_compiler__FnDef->retTy = ""; __c_compiler__FnDef->body = haki_array_new(sizeof(void*)); __c_compiler__FnDef; }); } __match_result; });
    if ((haki_array_length(f->body) != ((int64_t)2LL))) {
        haki_panic(haki_string_concat("expected 2 stmts, got: ", haki_int_to_string(haki_array_length(f->body))));
    }
}

void compiler__test_parse_multi_fn(void) {
    const char* src = "fn a(x: int) -> int { return x }\nfn b(y: int) -> int { return y }";
    __Tuple2* __mb_53917 = (__Tuple2*)(compiler__parse(src));
    void* items = (void*)__mb_53917->f0;
    void* err = (void*)__mb_53917->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("parse error: ", haki_error_message(err)));
    }
    if ((haki_array_length(items) != ((int64_t)2LL))) {
        haki_panic(haki_string_concat("expected 2 items, got: ", haki_int_to_string(haki_array_length(items))));
    }
}

compiler__FnDef* compiler__emptyFnDef(void) {
    void* params = haki_array_new(sizeof(void*));
    void* body = haki_array_new(sizeof(void*));
    return ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = ""; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = ""; __c_compiler__FnDef->body = body; __c_compiler__FnDef; });
}

compiler__StructDef* compiler__emptyStructDef(void) {
    void* fields = haki_array_new(sizeof(void*));
    return ({ compiler__StructDef* __c_compiler__StructDef = (compiler__StructDef*)malloc(sizeof(compiler__StructDef)); __c_compiler__StructDef->name = ""; __c_compiler__StructDef->fields = fields; __c_compiler__StructDef; });
}

compiler__FnDef* compiler__makeFnDef(const char* name, void* params, const char* retTy, void* body) {
    return ({ compiler__FnDef* __c_compiler__FnDef = (compiler__FnDef*)malloc(sizeof(compiler__FnDef)); __c_compiler__FnDef->name = name; __c_compiler__FnDef->params = params; __c_compiler__FnDef->retTy = retTy; __c_compiler__FnDef->body = body; __c_compiler__FnDef; });
}

compiler__Expr* compiler__nullExpr(void) {
    return ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = "__null__"; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = __pl; __ev; });
}

int8_t compiler__isNullExpr(compiler__Expr* e) {
    const char* n = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 4LL) { const char* n = *(const char**)__mpayload; __match_result = n; } else { __match_result = ""; } __match_result; });
    return (strcmp(n, "__null__") == 0);
}

void* compiler__emptyStmts(void) {
    void* s = haki_array_new(sizeof(void*));
    return s;
}

void* compiler__emptyExprs(void) {
    void* s = haki_array_new(sizeof(void*));
    return s;
}

void* compiler__emptyArms(void) {
    void* s = haki_array_new(sizeof(void*));
    return s;
}

mono__MonoProgram* mono__monoNew(void) {
    return ({ mono__MonoProgram* __c_mono__MonoProgram = (mono__MonoProgram*)malloc(sizeof(mono__MonoProgram)); __c_mono__MonoProgram->fns = haki_array_new(sizeof(void*)); __c_mono__MonoProgram->structs = haki_array_new(sizeof(void*)); __c_mono__MonoProgram->items = ((int64_t)0LL); __c_mono__MonoProgram->fnCount = ((int64_t)0LL); __c_mono__MonoProgram; });
}

const char* mono__mangle(const char* typeName, const char* methodName) {
    return haki_string_concat(haki_string_concat(typeName, "__"), methodName);
}

int8_t mono__isGenericTy(const char* ty) {
    if ((haki_string_length(ty) != ((int64_t)1LL))) {
        return 0;
    }
    const char* ch = haki_string_substring(ty, ((int64_t)0LL), ((int64_t)1LL));
    if ((((((strcmp(ch, "T") == 0) || (strcmp(ch, "U") == 0)) || (strcmp(ch, "V") == 0)) || (strcmp(ch, "K") == 0)) || (strcmp(ch, "E") == 0))) {
        return 1;
    }
    return 0;
}

int8_t mono__hasGenericParams(compiler__FnDef* f) {
    { void* __arr_p = f->params;
        int64_t __len_p = haki_array_length(__arr_p);
        for (int64_t __i_p = 0; __i_p < __len_p; __i_p++) {
            compiler__Param* p = *(compiler__Param**)haki_array_get(__arr_p, __i_p);
            if (mono__isGenericTy(p->ty)) {
                return 1;
            }
        }
    }
    if (mono__isGenericTy(f->retTy)) {
        return 1;
    }
    return 0;
}

mono__MonoFn* mono__lowerFn(compiler__FnDef* f) {
    return ({ mono__MonoFn* __c_mono__MonoFn = (mono__MonoFn*)malloc(sizeof(mono__MonoFn)); __c_mono__MonoFn->name = f->name; __c_mono__MonoFn->params = f->params; __c_mono__MonoFn->retTy = f->retTy; __c_mono__MonoFn->body = f->body; __c_mono__MonoFn; });
}

void mono__lowerStructMethods(mono__MonoProgram* prog, compiler__StructDef* s, void* body) {
    haki_array_append_val(prog->structs, &(s));
}

compiler__FnDef* mono__extractFn(compiler__Item* item) {
    compiler__FnDef* result = ({ compiler__FnDef* __match_result;  void* __msc = (void*)item;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = f; } else { __match_result = compiler__emptyFnDef(); } __match_result; });
    return result;
}

compiler__StructDef* mono__extractStruct(compiler__Item* item) {
    compiler__StructDef* result = ({ compiler__StructDef* __match_result;  void* __msc = (void*)item;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = s; } else { __match_result = compiler__emptyStructDef(); } __match_result; });
    return result;
}

void mono__lowerItem(mono__MonoProgram* prog, compiler__Item* item) {
    (prog->items = (prog->items + ((int64_t)1LL)));
    const char* tag = ({ const char* __match_result;  void* __msc = (void*)item;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = "fn"; } else if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = "struct"; } else if (__mtag == 2LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* a = *(const char**)((void**)__mpayload)[1]; __match_result = "import"; } __match_result; });
    if ((strcmp(tag, "fn") == 0)) {
        compiler__FnDef* f = mono__extractFn(item);
        if (mono__hasGenericParams(f)) {
            return;
        }
        { mono__MonoFn* __append_tmp = (mono__lowerFn(f)); haki_array_append_val(prog->fns, &__append_tmp); };
        (prog->fnCount = (prog->fnCount + ((int64_t)1LL)));
    }
    if ((strcmp(tag, "struct") == 0)) {
        compiler__StructDef* s = mono__extractStruct(item);
        haki_array_append_val(prog->structs, &(s));
    }
}

mono__MonoProgram* mono__monomorphize(void* items) {
    mono__MonoProgram* prog = mono__monoNew();
    { void* __arr_item = items;
        int64_t __len_item = haki_array_length(__arr_item);
        for (int64_t __i_item = 0; __i_item < __len_item; __i_item++) {
            compiler__Item* item = *(compiler__Item**)haki_array_get(__arr_item, __i_item);
            mono__lowerItem(prog, item);
        }
    }
    return prog;
}

void* mono__monoFromSource(const char* src) {
    __Tuple2* __mb_4930 = (__Tuple2*)(compiler__parse(src));
    void* items = (void*)__mb_4930->f0;
    void* parseErr = (void*)__mb_4930->f1;
    if ((parseErr != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(mono__monoNew());
        __ret->f1 = (void*)(haki_error_new(haki_string_concat("parse error: ", haki_error_message(parseErr))));
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(mono__monomorphize(items));
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void mono__showMonoProgram(mono__MonoProgram* prog) {
    haki_print("MonoProgram:");
    haki_print(haki_string_concat(haki_string_concat("  ", haki_int_to_string(prog->fnCount)), " concrete function(s)"));
    haki_print(haki_string_concat(haki_string_concat("  ", haki_int_to_string(haki_array_length(prog->structs))), " struct(s)"));
    { void* __arr_f = prog->fns;
        int64_t __len_f = haki_array_length(__arr_f);
        for (int64_t __i_f = 0; __i_f < __len_f; __i_f++) {
            mono__MonoFn* f = *(mono__MonoFn**)haki_array_get(__arr_f, __i_f);
            haki_print(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("  fn ", f->name), "("), haki_int_to_string(haki_array_length(f->params))), " params) -> "), f->retTy));
        }
    }
}

void mono__test_concrete_fn(void) {
    __Tuple2* __mb_5934 = (__Tuple2*)(mono__monoFromSource("fn add(a: int, b: int) -> int { return a + b }\nfn main() { }"));
    mono__MonoProgram* prog = (mono__MonoProgram*)__mb_5934->f0;
    void* err = (void*)__mb_5934->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("mono error: ", haki_error_message(err)));
    }
    if ((prog->fnCount != ((int64_t)2LL))) {
        haki_panic(haki_string_concat("expected 2 fns, got: ", haki_int_to_string(prog->fnCount)));
    }
}

void mono__test_struct_passthrough(void) {
    __Tuple2* __mb_6233 = (__Tuple2*)(mono__monoFromSource("struct Point { const x: int  const y: int }\nfn main() { }"));
    mono__MonoProgram* prog = (mono__MonoProgram*)__mb_6233->f0;
    void* err = (void*)__mb_6233->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("mono error: ", haki_error_message(err)));
    }
    if ((haki_array_length(prog->structs) != ((int64_t)1LL))) {
        haki_panic("expected 1 struct");
    }
    if ((prog->fnCount != ((int64_t)1LL))) {
        haki_panic("expected 1 fn");
    }
}

void mono__test_mangle(void) {
    const char* name = mono__mangle("Point", "distance");
    if ((!(strcmp(name, "Point__distance") == 0))) {
        haki_panic(haki_string_concat("mangle wrong: ", name));
    }
}

void mono__test_generic_skip(void) {
    __Tuple2* __mb_6683 = (__Tuple2*)(mono__monoFromSource("fn identity(x: T) -> T { return x }\nfn add(a: int, b: int) -> int { return a + b }"));
    mono__MonoProgram* prog = (mono__MonoProgram*)__mb_6683->f0;
    void* err = (void*)__mb_6683->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("mono error: ", haki_error_message(err)));
    }
    if ((prog->fnCount != ((int64_t)1LL))) {
        haki_panic(haki_string_concat("expected 1 concrete fn, got: ", haki_int_to_string(prog->fnCount)));
    }
}

void mono__main(void) {
    __Tuple2* __mb_7063 = (__Tuple2*)(mono__monoFromSource(haki_string_concat(haki_string_concat(haki_string_concat("fn add(a: int, b: int) -> int { return a + b }\n", "fn greet(name: string) -> string { return name }\n"), "struct Point { const x: int  const y: int }\n"), "fn main() { const x = add(1, 2)  print_int(x) }")));
    mono__MonoProgram* prog = (mono__MonoProgram*)__mb_7063->f0;
    void* err = (void*)__mb_7063->f1;
    if ((err != NULL)) {
        haki_print(haki_string_concat("error: ", haki_error_message(err)));
        return;
    }
    mono__showMonoProgram(prog);
}

mono__MonoFn* mono__makeMonoFn(const char* name, void* params, const char* retTy, void* body) {
    return ({ mono__MonoFn* __c_mono__MonoFn = (mono__MonoFn*)malloc(sizeof(mono__MonoFn)); __c_mono__MonoFn->name = name; __c_mono__MonoFn->params = params; __c_mono__MonoFn->retTy = retTy; __c_mono__MonoFn->body = body; __c_mono__MonoFn; });
}

void mono__mergeProgramWithAlias(mono__MonoProgram* dst, mono__MonoProgram* src, const char* alias) {
    { void* __arr_f = src->fns;
        int64_t __len_f = haki_array_length(__arr_f);
        for (int64_t __i_f = 0; __i_f < __len_f; __i_f++) {
            mono__MonoFn* f = *(mono__MonoFn**)haki_array_get(__arr_f, __i_f);
            mono__MonoFn* prefixed = (mono__MonoFn*)malloc(sizeof(mono__MonoFn));
            prefixed->name = haki_string_concat(haki_string_concat(alias, "__"), f->name);
            prefixed->params = f->params;
            prefixed->retTy = f->retTy;
            prefixed->body = f->body;
            haki_array_append_val(dst->fns, &(prefixed));
        }
    }
    { void* __arr_s = src->structs;
        int64_t __len_s = haki_array_length(__arr_s);
        for (int64_t __i_s = 0; __i_s < __len_s; __i_s++) {
            compiler__StructDef* s = *(compiler__StructDef**)haki_array_get(__arr_s, __i_s);
            haki_array_append_val(dst->structs, &(s));
        }
    }
}

const char* typeck__tyName(typeck__SemTy* t) {
    const char* s = ({ const char* __match_result;  void* __msc = (void*)t;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { __match_result = "int"; } else if (__mtag == 1LL) { __match_result = "float"; } else if (__mtag == 2LL) { __match_result = "bool"; } else if (__mtag == 3LL) { __match_result = "string"; } else if (__mtag == 4LL) { __match_result = "void"; } else if (__mtag == 5LL) { const char* n = *(const char**)__mpayload; __match_result = n; } else if (__mtag == 6LL) { const char* n = *(const char**)__mpayload; __match_result = haki_string_concat(n, "?"); } else if (__mtag == 7LL) { const char* n = *(const char**)__mpayload; __match_result = haki_string_concat(haki_string_concat("Array<", n), ">"); } else if (__mtag == 8LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* r = *(const char**)((void**)__mpayload)[1]; __match_result = haki_string_concat(haki_string_concat(haki_string_concat("fn(", p), ") -> "), r); } else if (__mtag == 9LL) { __match_result = "<error>"; } __match_result; });
    return s;
}

int8_t typeck__tyEq(typeck__SemTy* a, typeck__SemTy* b) {
    return (strcmp(typeck__tyName(a), typeck__tyName(b)) == 0);
}

typeck__SymTable* typeck__symNew(void) {
    return ({ typeck__SymTable* __c_typeck__SymTable = (typeck__SymTable*)malloc(sizeof(typeck__SymTable)); __c_typeck__SymTable->fns = haki_map_new(sizeof(void*)); __c_typeck__SymTable->structs = haki_map_new(sizeof(void*)); __c_typeck__SymTable->errors = haki_array_new(sizeof(void*)); __c_typeck__SymTable; });
}

void typeck__symError(typeck__SymTable* sym, const char* msg) {
    haki_array_append_val(sym->errors, &(msg));
}

void typeck__symRegisterFn(typeck__SymTable* sym, typeck__FnInfo* info) {
    haki_map_set(sym->fns, info->name, info);
}

void* typeck__symLookupFn(typeck__SymTable* sym, const char* name) {
    if (haki_map_has(sym->fns, name)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(haki_map_get_or_default(sym->fns, name, (void*)(({ typeck__FnInfo* __c_typeck__FnInfo = (typeck__FnInfo*)malloc(sizeof(typeck__FnInfo)); __c_typeck__FnInfo->name = ""; __c_typeck__FnInfo->retTy = ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 9LL; ((void**)__ev)[1] = NULL; __ev; }); __c_typeck__FnInfo->nParams = ((int64_t)0LL); __c_typeck__FnInfo; }))));
        { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 1; __ret->f1 = __f1; }
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(({ typeck__FnInfo* __c_typeck__FnInfo = (typeck__FnInfo*)malloc(sizeof(typeck__FnInfo)); __c_typeck__FnInfo->name = ""; __c_typeck__FnInfo->retTy = ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 9LL; ((void**)__ev)[1] = NULL; __ev; }); __c_typeck__FnInfo->nParams = ((int64_t)0LL); __c_typeck__FnInfo; }));
    { int8_t* __f1 = (int8_t*)malloc(sizeof(int8_t)); *__f1 = 0; __ret->f1 = __f1; }
    return __ret;
}

typeck__SemTy* typeck__resolveSimpleTy(const char* s) {
    if ((strcmp(s, "int") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(s, "float") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 1LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(s, "bool") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(s, "string") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 3LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(s, "void") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    int64_t n = haki_string_length(s);
    if (((n > ((int64_t)1LL)) && (strcmp(haki_string_substring(s, (n - ((int64_t)1LL)), n), "?") == 0))) {
        return ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = haki_string_substring(s, ((int64_t)0LL), (n - ((int64_t)1LL))); void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 6LL; ((void**)__ev)[1] = __pl; __ev; });
    }
    if (((n > ((int64_t)6LL)) && (strcmp(haki_string_substring(s, ((int64_t)0LL), ((int64_t)6LL)), "Array<") == 0))) {
        const char* inner = haki_string_substring(s, ((int64_t)6LL), (n - ((int64_t)1LL)));
        return ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = inner; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 7LL; ((void**)__ev)[1] = __pl; __ev; });
    }
    return ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = s; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 5LL; ((void**)__ev)[1] = __pl; __ev; });
}

void typeck__collectItem(typeck__SymTable* sym, compiler__Item* item) {
    int8_t isFn = ({ int8_t __match_result;  void* __msc = (void*)item;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = 1; } else if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = 0; } else if (__mtag == 2LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* a = *(const char**)((void**)__mpayload)[1]; __match_result = 0; } __match_result; });
    (void)(isFn);
}

void typeck__collectItems(typeck__SymTable* sym, void* items) {
    { void* __arr_item = items;
        int64_t __len_item = haki_array_length(__arr_item);
        for (int64_t __i_item = 0; __i_item < __len_item; __i_item++) {
            compiler__Item* item = *(compiler__Item**)haki_array_get(__arr_item, __i_item);
            typeck__collectItem(sym, item);
        }
    }
}

typeck__SemTy* typeck__inferExpr(typeck__SymTable* sym, compiler__Expr* e) {
    return typeck__inferExprInner(sym, e);
}

typeck__SemTy* typeck__inferExprInner(typeck__SymTable* sym, compiler__Expr* e) {
    const char* tag = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int64_t n = *(int64_t*)__mpayload; __match_result = "int"; } else if (__mtag == 1LL) { int8_t b = *(int8_t*)__mpayload; __match_result = "bool"; } else if (__mtag == 2LL) { __match_result = "null"; } else if (__mtag == 3LL) { const char* s = *(const char**)__mpayload; __match_result = "string"; } else if (__mtag == 4LL) { const char* n = *(const char**)__mpayload; __match_result = "ident"; } else if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "unary"; } else if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = "binary"; } else if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = "call"; } else if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = "field"; } else if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = "method"; } else if (__mtag == 10LL) { compiler__Expr* a = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* i = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "index"; } else if (__mtag == 11LL) { void* elems = *(void**)__mpayload; __match_result = "array"; } else if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = "if"; } else if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = "match"; } else if (__mtag == 14LL) { void* stmts = *(void**)__mpayload; __match_result = "block"; } else if (__mtag == 15LL) { compiler__Expr* inner = *(compiler__Expr**)__mpayload; __match_result = "async"; } __match_result; });
    if ((strcmp(tag, "int") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(tag, "bool") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(tag, "null") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(tag, "string") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 3LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(tag, "unary") == 0)) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    if ((strcmp(tag, "binary") == 0)) {
        return typeck__inferBinary(sym, e);
    }
    if ((strcmp(tag, "call") == 0)) {
        return typeck__inferCall(sym, e);
    }
    return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 4LL; ((void**)__ev)[1] = NULL; __ev; });
}

typeck__SemTy* typeck__inferBinary(typeck__SymTable* sym, compiler__Expr* e) {
    const char* op = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = op; } else { __match_result = ""; } __match_result; });
    if ((((((strcmp(op, "+") == 0) || (strcmp(op, "-") == 0)) || (strcmp(op, "*") == 0)) || (strcmp(op, "/") == 0)) || (strcmp(op, "%") == 0))) {
        return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 0LL; ((void**)__ev)[1] = NULL; __ev; });
    }
    return ({ typeck__SemTy* __ev = (typeck__SemTy*)malloc(sizeof(int64_t)*2); ((int64_t*)__ev)[0] = 2LL; ((void**)__ev)[1] = NULL; __ev; });
}

typeck__SemTy* typeck__inferCall(typeck__SymTable* sym, compiler__Expr* e) {
    const char* name = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = n; } else { __match_result = ""; } __match_result; });
    void* args = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = args; } else { __match_result = haki_array_new(sizeof(void*)); } __match_result; });
    __Tuple2* __mb_5910 = (__Tuple2*)(typeck__symLookupFn(sym, name));
    typeck__FnInfo* info = (typeck__FnInfo*)__mb_5910->f0;
    int8_t found = *(int8_t*)__mb_5910->f1;
    if (found) {
        if ((haki_array_length(args) != info->nParams)) {
            typeck__symError(sym, haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("call to ", name), ": expected "), haki_int_to_string(info->nParams)), " args, got "), haki_int_to_string(haki_array_length(args))));
        }
        return info->retTy;
    }
    return ({ const char** __pl = (const char**)malloc(sizeof(const char*)); *__pl = name; void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = 5LL; ((void**)__ev)[1] = __pl; __ev; });
}

void typeck__checkStmts(typeck__SymTable* sym, void* stmts, const char* fnName, typeck__SemTy* retTy) {
    { void* __arr_stmt = stmts;
        int64_t __len_stmt = haki_array_length(__arr_stmt);
        for (int64_t __i_stmt = 0; __i_stmt < __len_stmt; __i_stmt++) {
            compiler__Stmt* stmt = *(compiler__Stmt**)haki_array_get(__arr_stmt, __i_stmt);
            typeck__checkStmt(sym, stmt, fnName, retTy);
        }
    }
}

void typeck__checkStmt(typeck__SymTable* sym, compiler__Stmt* stmt, const char* fnName, typeck__SemTy* retTy) {
    (void)(({ int64_t __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = ((int64_t)0LL); } else if (__mtag == 1LL) { void* vals = *(void**)__mpayload; __match_result = ((int64_t)0LL); } else if (__mtag == 2LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = ((int64_t)0LL); } else if (__mtag == 3LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = ((int64_t)0LL); } else if (__mtag == 6LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = ((int64_t)0LL); } else if (__mtag == 4LL) { __match_result = ((int64_t)0LL); } else if (__mtag == 5LL) { __match_result = ((int64_t)0LL); } else if (__mtag == 10LL) { compiler__Expr* target = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* val = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = ((int64_t)0LL); } else if (__mtag == 7LL) { compiler__Expr* cond = *(compiler__Expr**)((void**)__mpayload)[0]; void* then = *(void**)((void**)__mpayload)[1]; void* els = *(void**)((void**)__mpayload)[2]; __match_result = ((int64_t)0LL); } else if (__mtag == 8LL) { compiler__Expr* cond = *(compiler__Expr**)((void**)__mpayload)[0]; void* body = *(void**)((void**)__mpayload)[1]; __match_result = ((int64_t)0LL); } else if (__mtag == 9LL) { const char* varName = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = ((int64_t)0LL); } __match_result; }));
}

void typeck__checkFn(typeck__SymTable* sym, compiler__FnDef* f) {
    typeck__SemTy* retTy = typeck__resolveSimpleTy(f->retTy);
    typeck__checkStmts(sym, f->body, f->name, retTy);
}

void typeck__checkItems(typeck__SymTable* sym, void* items) {
    { void* __arr_item = items;
        int64_t __len_item = haki_array_length(__arr_item);
        for (int64_t __i_item = 0; __i_item < __len_item; __i_item++) {
            compiler__Item* item = *(compiler__Item**)haki_array_get(__arr_item, __i_item);
            (void)(({ int64_t __match_result;  void* __msc = (void*)item;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { compiler__FnDef* f = *(compiler__FnDef**)__mpayload; __match_result = ((int64_t)0LL); } else if (__mtag == 1LL) { compiler__StructDef* s = *(compiler__StructDef**)__mpayload; __match_result = ((int64_t)0LL); } else if (__mtag == 2LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* a = *(const char**)((void**)__mpayload)[1]; __match_result = ((int64_t)0LL); } __match_result; }));
        }
    }
}

void* typeck__typecheck(const char* src) {
    __Tuple2* __mb_8345 = (__Tuple2*)(compiler__parse(src));
    void* items = (void*)__mb_8345->f0;
    void* parseErr = (void*)__mb_8345->f1;
    if ((parseErr != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(typeck__symNew());
        __ret->f1 = (void*)(haki_error_new(haki_string_concat("parse error: ", haki_error_message(parseErr))));
        return __ret;
    }
    typeck__SymTable* sym = typeck__symNew();
    typeck__collectItems(sym, items);
    typeck__checkItems(sym, items);
    if ((haki_array_length(sym->errors) > ((int64_t)0LL))) {
        const char* msg = "typecheck errors:\n";
        { void* __arr_e = sym->errors;
            int64_t __len_e = haki_array_length(__arr_e);
            for (int64_t __i_e = 0; __i_e < __len_e; __i_e++) {
                const char* e = *(const char**)haki_array_get(__arr_e, __i_e);
                (msg = haki_string_concat(haki_string_concat(haki_string_concat(msg, "  - "), e), "\n"));
            }
        }
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)(sym);
        __ret->f1 = (void*)(haki_error_new(msg));
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(sym);
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void typeck__test_collect_fns(void) {
    __Tuple2* __mb_9038 = (__Tuple2*)(typeck__typecheck("fn add(a: int, b: int) -> int { return a + b }\nfn main() { print(\"hi\") }"));
    typeck__SymTable* sym = (typeck__SymTable*)__mb_9038->f0;
    void* err = (void*)__mb_9038->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("typecheck error: ", haki_error_message(err)));
    }
    __Tuple2* __mb_9212 = (__Tuple2*)(typeck__symLookupFn(sym, "add"));
    typeck__FnInfo* info = (typeck__FnInfo*)__mb_9212->f0;
    int8_t found = *(int8_t*)__mb_9212->f1;
    if ((!found)) {
        haki_panic("add not found");
    }
    if ((info->nParams != ((int64_t)2LL))) {
        haki_panic("expected 2 params");
    }
    if ((!(strcmp(typeck__tyName(info->retTy), "int") == 0))) {
        haki_panic("expected int return");
    }
}

void typeck__test_wrong_arg_count(void) {
    __Tuple2* __mb_9456 = (__Tuple2*)(typeck__typecheck("fn add(a: int, b: int) -> int { return a + b }\nfn main() { const x = add(1) }"));
    typeck__SymTable* sym = (typeck__SymTable*)__mb_9456->f0;
    void* err = (void*)__mb_9456->f1;
    if ((err == NULL)) {
        haki_panic("expected typecheck error for wrong arg count");
    }
}

void typeck__test_collect_struct(void) {
    __Tuple2* __mb_9676 = (__Tuple2*)(typeck__typecheck("struct Point { const x: int  const y: int }\nfn main() { }"));
    typeck__SymTable* sym = (typeck__SymTable*)__mb_9676->f0;
    void* err = (void*)__mb_9676->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("typecheck error: ", haki_error_message(err)));
    }
    int8_t hasPoint = haki_map_has(sym->structs, "Point");
    if ((!hasPoint)) {
        haki_panic("Point not collected");
    }
}

void typeck__test_complex_fn(void) {
    const char* src = "fn fib(n: int) -> int {\n  if n < 2 { return n }\n  return fib(n - 1) + fib(n - 2)\n}\nfn main() { print_int(fib(10)) }";
    __Tuple2* __mb_10093 = (__Tuple2*)(typeck__typecheck(src));
    typeck__SymTable* sym = (typeck__SymTable*)__mb_10093->f0;
    void* err = (void*)__mb_10093->f1;
    if ((err != NULL)) {
        haki_panic(haki_string_concat("typecheck error: ", haki_error_message(err)));
    }
    __Tuple2* __mb_10193 = (__Tuple2*)(typeck__symLookupFn(sym, "fib"));
    typeck__FnInfo* info = (typeck__FnInfo*)__mb_10193->f0;
    int8_t found = *(int8_t*)__mb_10193->f1;
    if ((!found)) {
        haki_panic("fib not found");
    }
}

void typeck__main(void) {
    __Tuple2* __mb_10297 = (__Tuple2*)(typeck__typecheck(haki_string_concat(haki_string_concat("fn add(a: int, b: int) -> int { return a + b }\n", "fn greet(name: string) -> string { return name }\n"), "fn main() { const x = add(1, 2)  print_int(x) }")));
    typeck__SymTable* sym = (typeck__SymTable*)__mb_10297->f0;
    void* err = (void*)__mb_10297->f1;
    if ((err != NULL)) {
        haki_print(haki_string_concat("error: ", haki_error_message(err)));
        return;
    }
    haki_print(haki_string_concat(haki_string_concat("collected ", haki_int_to_string(haki_map_length(sym->fns))), " functions"));
    __Tuple2* __mb_10669 = (__Tuple2*)(typeck__symLookupFn(sym, "add"));
    typeck__FnInfo* addInfo = (typeck__FnInfo*)__mb_10669->f0;
    int8_t found = *(int8_t*)__mb_10669->f1;
    if (found) {
        haki_print(haki_string_concat(haki_string_concat(haki_string_concat("add: ", haki_int_to_string(addInfo->nParams)), " params -> "), typeck__tyName(addInfo->retTy)));
    }
}

const char* typeck__semTyToStr(typeck__SemTy* ty) {
    return ({ const char* __match_result;  void* __msc = (void*)ty;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { __match_result = "int"; } else if (__mtag == 1LL) { __match_result = "float"; } else if (__mtag == 2LL) { __match_result = "bool"; } else if (__mtag == 3LL) { __match_result = "string"; } else if (__mtag == 4LL) { __match_result = "void"; } else if (__mtag == 5LL) { const char* n = *(const char**)__mpayload; __match_result = n; } else if (__mtag == 6LL) { const char* n = *(const char**)__mpayload; __match_result = haki_string_concat(n, "?"); } else if (__mtag == 7LL) { const char* n = *(const char**)__mpayload; __match_result = haki_string_concat(haki_string_concat("Array<", n), ">"); } else if (__mtag == 8LL) { const char* p = *(const char**)((void**)__mpayload)[0]; const char* r = *(const char**)((void**)__mpayload)[1]; __match_result = haki_string_concat(haki_string_concat(haki_string_concat("fn(", p), ")->"), r); } else if (__mtag == 9LL) { __match_result = "void*"; } __match_result; });
}

tinfer__Scope* tinfer__scopeNew(void) {
    return ({ tinfer__Scope* __c_tinfer__Scope = (tinfer__Scope*)malloc(sizeof(tinfer__Scope)); __c_tinfer__Scope->names = haki_array_new(sizeof(void*)); __c_tinfer__Scope->types = haki_array_new(sizeof(void*)); __c_tinfer__Scope; });
}

void tinfer__scopeSet(tinfer__Scope* sc, const char* name, const char* ty) {
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(sc->names))) {
        if ((strcmp((*(const char**)haki_array_get(sc->names, i)), name) == 0)) {
            ((*(const char**)haki_array_get(sc->types, i)) = ty);
            return;
        }
        (i = (i + ((int64_t)1LL)));
    }
    haki_array_append_val(sc->names, &(name));
    haki_array_append_val(sc->types, &(ty));
}

const char* tinfer__scopeGet(tinfer__Scope* sc, const char* name) {
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(sc->names))) {
        if ((strcmp((*(const char**)haki_array_get(sc->names, i)), name) == 0)) {
            return (*(const char**)haki_array_get(sc->types, i));
        }
        (i = (i + ((int64_t)1LL)));
    }
    return "void*";
}

tinfer__Scope* tinfer__scopeCopy(tinfer__Scope* sc) {
    tinfer__Scope* copy = tinfer__scopeNew();
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(sc->names))) {
        haki_array_append_val(copy->names, &((*(const char**)haki_array_get(sc->names, i))));
        haki_array_append_val(copy->types, &((*(const char**)haki_array_get(sc->types, i))));
        (i = (i + ((int64_t)1LL)));
    }
    return copy;
}

int8_t tinfer__isIntTy(const char* ty) {
    return (strcmp(ty, "int") == 0);
}

int8_t tinfer__isFloatTy(const char* ty) {
    return (strcmp(ty, "float") == 0);
}

int8_t tinfer__isStringTy(const char* ty) {
    return (strcmp(ty, "string") == 0);
}

int8_t tinfer__isBoolTy(const char* ty) {
    return (strcmp(ty, "bool") == 0);
}

int8_t tinfer__isNumericTy(const char* ty) {
    if ((strcmp(ty, "int") == 0)) {
        return 1;
    }
    if ((strcmp(ty, "float") == 0)) {
        return 1;
    }
    return 0;
}

const char* tinfer__binaryOpC(const char* op, const char* ty) {
    if (((strcmp(op, "+") == 0) && tinfer__isNumericTy(ty))) {
        return "+";
    }
    if (((strcmp(op, "-") == 0) && tinfer__isNumericTy(ty))) {
        return "-";
    }
    if (((strcmp(op, "*") == 0) && tinfer__isNumericTy(ty))) {
        return "*";
    }
    if (((strcmp(op, "/") == 0) && tinfer__isNumericTy(ty))) {
        return "/";
    }
    if (((strcmp(op, "%") == 0) && tinfer__isNumericTy(ty))) {
        return "%";
    }
    if (((strcmp(op, "<") == 0) && tinfer__isNumericTy(ty))) {
        return "<";
    }
    if (((strcmp(op, ">") == 0) && tinfer__isNumericTy(ty))) {
        return ">";
    }
    if (((strcmp(op, "<=") == 0) && tinfer__isNumericTy(ty))) {
        return "<=";
    }
    if (((strcmp(op, ">=") == 0) && tinfer__isNumericTy(ty))) {
        return ">=";
    }
    if (((strcmp(op, "==") == 0) && tinfer__isNumericTy(ty))) {
        return "==";
    }
    if (((strcmp(op, "!=") == 0) && tinfer__isNumericTy(ty))) {
        return "!=";
    }
    if (((strcmp(op, "==") == 0) && tinfer__isBoolTy(ty))) {
        return "==";
    }
    if (((strcmp(op, "!=") == 0) && tinfer__isBoolTy(ty))) {
        return "!=";
    }
    if ((strcmp(op, "&&") == 0)) {
        return "&&";
    }
    if ((strcmp(op, "||") == 0)) {
        return "||";
    }
    if (((strcmp(op, "+") == 0) && tinfer__isStringTy(ty))) {
        return "string_concat";
    }
    if (((strcmp(op, "==") == 0) && tinfer__isStringTy(ty))) {
        return "string_eq";
    }
    if (((strcmp(op, "!=") == 0) && tinfer__isStringTy(ty))) {
        return "string_neq";
    }
    if ((strcmp(op, "==") == 0)) {
        return "ptr_eq";
    }
    if ((strcmp(op, "!=") == 0)) {
        return "ptr_neq";
    }
    return op;
}

const char* tinfer__builtinReturnTy(const char* name) {
    if ((strcmp(name, "print") == 0)) {
        return "void";
    }
    if ((strcmp(name, "print_int") == 0)) {
        return "void";
    }
    if ((strcmp(name, "print_float") == 0)) {
        return "void";
    }
    if ((strcmp(name, "print_bool") == 0)) {
        return "void";
    }
    if ((strcmp(name, "int_to_string") == 0)) {
        return "string";
    }
    if ((strcmp(name, "float_to_string") == 0)) {
        return "string";
    }
    if ((strcmp(name, "bool_to_string") == 0)) {
        return "string";
    }
    if ((strcmp(name, "string_length") == 0)) {
        return "int";
    }
    if ((strcmp(name, "string_concat") == 0)) {
        return "string";
    }
    if ((strcmp(name, "string_substring") == 0)) {
        return "string";
    }
    if ((strcmp(name, "string_contains") == 0)) {
        return "bool";
    }
    if ((strcmp(name, "string_split") == 0)) {
        return "string";
    }
    if ((strcmp(name, "string_to_upper") == 0)) {
        return "string";
    }
    if ((strcmp(name, "string_to_lower") == 0)) {
        return "string";
    }
    if ((strcmp(name, "string_trim") == 0)) {
        return "string";
    }
    if ((strcmp(name, "int_to_float") == 0)) {
        return "float";
    }
    if ((strcmp(name, "float_to_int") == 0)) {
        return "int";
    }
    if ((strcmp(name, "readFile") == 0)) {
        return "void*";
    }
    if ((strcmp(name, "writeFile") == 0)) {
        return "void*";
    }
    if ((strcmp(name, "haki_array_length") == 0)) {
        return "int";
    }
    if ((strcmp(name, "argv") == 0)) {
        return "void*";
    }
    if ((strcmp(name, "panic") == 0)) {
        return "void";
    }
    return "void*";
}

const char* tinfer__inferExprTy(compiler__Expr* e, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* tag = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int64_t n = *(int64_t*)__mpayload; __match_result = "int_lit"; } else if (__mtag == 1LL) { int8_t b = *(int8_t*)__mpayload; __match_result = "bool_lit"; } else if (__mtag == 2LL) { __match_result = "null"; } else if (__mtag == 3LL) { const char* s = *(const char**)__mpayload; __match_result = "string_lit"; } else if (__mtag == 4LL) { const char* n = *(const char**)__mpayload; __match_result = "ident"; } else if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "unary"; } else if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = "binary"; } else if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = "call"; } else if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = "field"; } else if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = "method"; } else if (__mtag == 10LL) { compiler__Expr* a = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* idx = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "index"; } else if (__mtag == 11LL) { void* elems = *(void**)__mpayload; __match_result = "array"; } else if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = "if"; } else if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = "match"; } else if (__mtag == 14LL) { void* stmts = *(void**)__mpayload; __match_result = "block"; } else if (__mtag == 15LL) { compiler__Expr* inner = *(compiler__Expr**)__mpayload; __match_result = "async"; } else { __match_result = "unknown"; } __match_result; });
    if ((strcmp(tag, "int_lit") == 0)) {
        return "int";
    }
    if ((strcmp(tag, "bool_lit") == 0)) {
        return "bool";
    }
    if ((strcmp(tag, "null") == 0)) {
        return "void*";
    }
    if ((strcmp(tag, "string_lit") == 0)) {
        return "string";
    }
    if ((strcmp(tag, "ident") == 0)) {
        const char* name = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 4LL) { const char* n = *(const char**)__mpayload; __match_result = n; } else { __match_result = ""; } __match_result; });
        return tinfer__scopeGet(sc, name);
    }
    if ((strcmp(tag, "unary") == 0)) {
        const char* op = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = op; } else { __match_result = ""; } __match_result; });
        compiler__Expr* inner = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = inner; } else { __match_result = compiler__nullExpr(); } __match_result; });
        if ((strcmp(op, "!") == 0)) {
            return "bool";
        }
        if ((strcmp(op, "-") == 0)) {
            const char* ity = tinfer__inferExprTy(inner, sc, sym);
            return ity;
        }
        return "void*";
    }
    if ((strcmp(tag, "binary") == 0)) {
        const char* op = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = op; } else { __match_result = ""; } __match_result; });
        compiler__Expr* l = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = l; } else { __match_result = compiler__nullExpr(); } __match_result; });
        const char* lty = tinfer__inferExprTy(l, sc, sym);
        if (((strcmp(op, "&&") == 0) || (strcmp(op, "||") == 0))) {
            return "bool";
        }
        if (((strcmp(op, "==") == 0) || (strcmp(op, "!=") == 0))) {
            return "bool";
        }
        if (((strcmp(op, "<") == 0) || (strcmp(op, ">") == 0))) {
            return "bool";
        }
        if (((strcmp(op, "<=") == 0) || (strcmp(op, ">=") == 0))) {
            return "bool";
        }
        if ((((((strcmp(op, "+") == 0) || (strcmp(op, "-") == 0)) || (strcmp(op, "*") == 0)) || (strcmp(op, "/") == 0)) || (strcmp(op, "%") == 0))) {
            return lty;
        }
        return lty;
    }
    if ((strcmp(tag, "call") == 0)) {
        const char* name = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = n; } else { __match_result = ""; } __match_result; });
        const char* bty = tinfer__builtinReturnTy(name);
        if ((!(strcmp(bty, "void*") == 0))) {
            return bty;
        }
        __Tuple2* __mb_9363 = (__Tuple2*)(typeck__symLookupFn(sym, name));
        typeck__FnInfo* info = (typeck__FnInfo*)__mb_9363->f0;
        int8_t found = *(int8_t*)__mb_9363->f1;
        if (found) {
            const char* retTyStr = typeck__semTyToStr(info->retTy);
            if ((strcmp(retTyStr, "int") == 0)) {
                return "int";
            }
            if ((strcmp(retTyStr, "float") == 0)) {
                return "float";
            }
            if ((strcmp(retTyStr, "bool") == 0)) {
                return "bool";
            }
            if ((strcmp(retTyStr, "string") == 0)) {
                return "string";
            }
            if ((strcmp(retTyStr, "void") == 0)) {
                return "void";
            }
        }
        return "void*";
    }
    if ((strcmp(tag, "field") == 0)) {
        compiler__Expr* recv = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = recv; } else { __match_result = compiler__nullExpr(); } __match_result; });
        const char* f = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = f; } else { __match_result = ""; } __match_result; });
        if ((strcmp(f, "length") == 0)) {
            return "int";
        }
        if ((strcmp(f, "message") == 0)) {
            return "string";
        }
        return "void*";
    }
    if ((strcmp(tag, "method") == 0)) {
        const char* m = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = m; } else { __match_result = ""; } __match_result; });
        if ((strcmp(m, "length") == 0)) {
            return "int";
        }
        if ((strcmp(m, "substring") == 0)) {
            return "string";
        }
        if ((strcmp(m, "contains") == 0)) {
            return "bool";
        }
        if ((strcmp(m, "toUpper") == 0)) {
            return "string";
        }
        if ((strcmp(m, "toLower") == 0)) {
            return "string";
        }
        if ((strcmp(m, "trim") == 0)) {
            return "string";
        }
        return "void*";
    }
    if ((strcmp(tag, "index") == 0)) {
        return "void*";
    }
    if ((strcmp(tag, "array") == 0)) {
        return "void*";
    }
    if ((strcmp(tag, "async") == 0)) {
        return "void*";
    }
    if ((strcmp(tag, "if") == 0)) {
        void* th = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = th; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        if ((haki_array_length(th) > ((int64_t)0LL))) {
            compiler__Stmt* last = (*(compiler__Stmt**)haki_array_get(th, (haki_array_length(th) - ((int64_t)1LL))));
            const char* lastTy = tinfer__inferStmtYieldTy(last, sc, sym);
            if ((haki_string_length(lastTy) > ((int64_t)0LL))) {
                return lastTy;
            }
        }
        return "void*";
    }
    if ((strcmp(tag, "match") == 0)) {
        void* arms = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = arms; } else { __match_result = compiler__emptyArms(); } __match_result; });
        if ((haki_array_length(arms) > ((int64_t)0LL))) {
            compiler__MatchArm* firstArm = (*(compiler__MatchArm**)haki_array_get(arms, ((int64_t)0LL)));
            const char* armTy = tinfer__inferMatchArmTy(firstArm, sc, sym);
            if ((haki_string_length(armTy) > ((int64_t)0LL))) {
                return armTy;
            }
        }
        return "void*";
    }
    if ((strcmp(tag, "block") == 0)) {
        void* stmts = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 14LL) { void* stmts = *(void**)__mpayload; __match_result = stmts; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        if ((haki_array_length(stmts) > ((int64_t)0LL))) {
            compiler__Stmt* last = (*(compiler__Stmt**)haki_array_get(stmts, (haki_array_length(stmts) - ((int64_t)1LL))));
            const char* lastTy = tinfer__inferStmtYieldTy(last, sc, sym);
            if ((haki_string_length(lastTy) > ((int64_t)0LL))) {
                return lastTy;
            }
        }
        return "void*";
    }
    return "void*";
}

const char* tinfer__inferStmtYieldTy(compiler__Stmt* stmt, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* tag = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = "let"; } else if (__mtag == 1LL) { void* vals = *(void**)__mpayload; __match_result = "return"; } else if (__mtag == 2LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = "yield"; } else if (__mtag == 6LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = "expr"; } else { __match_result = "other"; } __match_result; });
    if ((strcmp(tag, "yield") == 0)) {
        compiler__Expr* e = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 2LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = e; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return tinfer__inferExprTy(e, sc, sym);
    }
    if ((strcmp(tag, "expr") == 0)) {
        compiler__Expr* e = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = e; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return tinfer__inferExprTy(e, sc, sym);
    }
    return "";
}

const char* tinfer__inferMatchArmTy(compiler__MatchArm* arm, tinfer__Scope* sc, typeck__SymTable* sym) {
    if ((haki_array_length(arm->body) > ((int64_t)0LL))) {
        compiler__Stmt* last = (*(compiler__Stmt**)haki_array_get(arm->body, (haki_array_length(arm->body) - ((int64_t)1LL))));
        return tinfer__inferStmtYieldTy(last, sc, sym);
    }
    return "";
}

tinfer__TypedExpr* tinfer__makeTyped(compiler__Expr* e, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* ty = tinfer__inferExprTy(e, sc, sym);
    return ({ tinfer__TypedExpr* __c_tinfer__TypedExpr = (tinfer__TypedExpr*)malloc(sizeof(tinfer__TypedExpr)); __c_tinfer__TypedExpr->kind = e; __c_tinfer__TypedExpr->ty = ty; __c_tinfer__TypedExpr; });
}

void tinfer__populateScopeFromParams(tinfer__Scope* sc, void* params) {
    { void* __arr_p = params;
        int64_t __len_p = haki_array_length(__arr_p);
        for (int64_t __i_p = 0; __i_p < __len_p; __i_p++) {
            compiler__Param* p = *(compiler__Param**)haki_array_get(__arr_p, __i_p);
            const char* ty = tinfer__simplifyTy(p->ty);
            tinfer__scopeSet(sc, p->name, ty);
        }
    }
}

const char* tinfer__simplifyTy(const char* ty) {
    if ((strcmp(ty, "int") == 0)) {
        return "int";
    }
    if ((strcmp(ty, "float") == 0)) {
        return "float";
    }
    if ((strcmp(ty, "bool") == 0)) {
        return "bool";
    }
    if ((strcmp(ty, "string") == 0)) {
        return "string";
    }
    if ((strcmp(ty, "void") == 0)) {
        return "void";
    }
    if (((haki_string_length(ty) > ((int64_t)5LL)) && (strcmp(haki_string_substring(ty, ((int64_t)0LL), ((int64_t)5LL)), "Array") == 0))) {
        return "void*";
    }
    if (((haki_string_length(ty) > ((int64_t)3LL)) && (strcmp(haki_string_substring(ty, ((int64_t)0LL), ((int64_t)3LL)), "Map") == 0))) {
        return "void*";
    }
    int64_t n = haki_string_length(ty);
    if (((n > ((int64_t)1LL)) && (strcmp(haki_string_substring(ty, (n - ((int64_t)1LL)), n), "?") == 0))) {
        return "void*";
    }
    return "void*";
}

void tinfer__inferFnScope(void* stmts, tinfer__Scope* sc, typeck__SymTable* sym) {
    { void* __arr_stmt = stmts;
        int64_t __len_stmt = haki_array_length(__arr_stmt);
        for (int64_t __i_stmt = 0; __i_stmt < __len_stmt; __i_stmt++) {
            compiler__Stmt* stmt = *(compiler__Stmt**)haki_array_get(__arr_stmt, __i_stmt);
            const char* tag = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = "let"; } else if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = "if"; } else if (__mtag == 8LL) { compiler__Expr* cond = *(compiler__Expr**)((void**)__mpayload)[0]; void* body = *(void**)((void**)__mpayload)[1]; __match_result = "while"; } else if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = "for"; } else { __match_result = "other"; } __match_result; });
            if ((strcmp(tag, "let") == 0)) {
                const char* name = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = name; } else { __match_result = ""; } __match_result; });
                compiler__Expr* init = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = init; } else { __match_result = compiler__nullExpr(); } __match_result; });
                const char* ty = tinfer__inferExprTy(init, sc, sym);
                tinfer__scopeSet(sc, name, ty);
            }
            if ((strcmp(tag, "if") == 0)) {
                void* th = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = th; } else { __match_result = compiler__emptyStmts(); } __match_result; });
                void* el = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = el; } else { __match_result = compiler__emptyStmts(); } __match_result; });
                tinfer__inferFnScope(th, sc, sym);
                tinfer__inferFnScope(el, sc, sym);
            }
            if ((strcmp(tag, "while") == 0)) {
                void* body = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* cond = *(compiler__Expr**)((void**)__mpayload)[0]; void* body = *(void**)((void**)__mpayload)[1]; __match_result = body; } else { __match_result = compiler__emptyStmts(); } __match_result; });
                tinfer__inferFnScope(body, sc, sym);
            }
            if ((strcmp(tag, "for") == 0)) {
                const char* v = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = v; } else { __match_result = ""; } __match_result; });
                void* body = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = body; } else { __match_result = compiler__emptyStmts(); } __match_result; });
                tinfer__scopeSet(sc, v, "void*");
                tinfer__inferFnScope(body, sc, sym);
            }
        }
    }
}

tinfer__TypedExpr* tinfer__inferExpr(compiler__Expr* e, compiler__FnDef* fnDef, typeck__SymTable* sym) {
    tinfer__Scope* sc = tinfer__scopeNew();
    tinfer__populateScopeFromParams(sc, fnDef->params);
    tinfer__inferFnScope(fnDef->body, sc, sym);
    return tinfer__makeTyped(e, sc, sym);
}

tinfer__Scope* tinfer__buildFnScope(compiler__FnDef* fnDef, typeck__SymTable* sym) {
    tinfer__Scope* sc = tinfer__scopeNew();
    tinfer__populateScopeFromParams(sc, fnDef->params);
    tinfer__inferFnScope(fnDef->body, sc, sym);
    return sc;
}

const char* tinfer__inferWithScope(compiler__Expr* e, tinfer__Scope* sc, typeck__SymTable* sym) {
    return tinfer__inferExprTy(e, sc, sym);
}

const char* cName(const char* name) {
    if ((strcmp(name, "int") == 0)) {
        return "haki_int";
    }
    if ((strcmp(name, "float") == 0)) {
        return "haki_float";
    }
    if ((strcmp(name, "double") == 0)) {
        return "haki_double";
    }
    if ((strcmp(name, "char") == 0)) {
        return "haki_char";
    }
    if ((strcmp(name, "return") == 0)) {
        return "haki_return";
    }
    if ((strcmp(name, "void") == 0)) {
        return "haki_void";
    }
    if ((strcmp(name, "struct") == 0)) {
        return "haki_struct";
    }
    if ((strcmp(name, "typedef") == 0)) {
        return "haki_typedef";
    }
    if ((strcmp(name, "static") == 0)) {
        return "haki_static";
    }
    if ((strcmp(name, "extern") == 0)) {
        return "haki_extern";
    }
    if ((strcmp(name, "switch") == 0)) {
        return "haki_switch";
    }
    if ((strcmp(name, "case") == 0)) {
        return "haki_case";
    }
    if ((strcmp(name, "default") == 0)) {
        return "haki_default";
    }
    if ((strcmp(name, "enum") == 0)) {
        return "haki_enum";
    }
    return name;
}

const char* cTy(const char* ty) {
    if ((strcmp(ty, "int") == 0)) {
        return "int64_t";
    }
    if ((strcmp(ty, "float") == 0)) {
        return "double";
    }
    if ((strcmp(ty, "bool") == 0)) {
        return "int8_t";
    }
    if ((strcmp(ty, "string") == 0)) {
        return "const char*";
    }
    if ((strcmp(ty, "void") == 0)) {
        return "void";
    }
    if ((strcmp(ty, "") == 0)) {
        return "void";
    }
    if ((strcmp(ty, "Error") == 0)) {
        return "void*";
    }
    if ((strcmp(ty, "Error?") == 0)) {
        return "void*";
    }
    int64_t n = haki_string_length(ty);
    if (((n > ((int64_t)1LL)) && (strcmp(haki_string_substring(ty, (n - ((int64_t)1LL)), n), "?") == 0))) {
        return "void*";
    }
    if (((haki_string_length(ty) > ((int64_t)5LL)) && (strcmp(haki_string_substring(ty, ((int64_t)0LL), ((int64_t)5LL)), "Array") == 0))) {
        return "void*";
    }
    if (((haki_string_length(ty) > ((int64_t)3LL)) && (strcmp(haki_string_substring(ty, ((int64_t)0LL), ((int64_t)3LL)), "Map") == 0))) {
        return "void*";
    }
    if (((haki_string_length(ty) > ((int64_t)4LL)) && (strcmp(haki_string_substring(ty, ((int64_t)0LL), ((int64_t)4LL)), "Task") == 0))) {
        return "void*";
    }
    int64_t di = ((int64_t)0LL);
    while ((di < haki_string_length(ty))) {
        if ((strcmp(haki_string_substring(ty, di, (di + ((int64_t)1LL))), ".") == 0)) {
            const char* prefix = haki_string_substring(ty, ((int64_t)0LL), di);
            const char* suffix = haki_string_substring(ty, (di + ((int64_t)1LL)), haki_string_length(ty));
            return haki_string_concat(haki_string_concat(haki_string_concat(prefix, "__"), suffix), "*");
        }
        (di = (di + ((int64_t)1LL)));
    }
    return haki_string_concat(cName(ty), "*");
}

const char* cRetTy(const char* ty) {
    if (((strcmp(ty, "void") == 0) || (strcmp(ty, "") == 0))) {
        return "void";
    }
    int64_t n = haki_string_length(ty);
    if (((n > ((int64_t)0LL)) && (strcmp(haki_string_substring(ty, ((int64_t)0LL), ((int64_t)1LL)), "(") == 0))) {
        return "void*";
    }
    return cTy(ty);
}

int8_t isScalarTy(const char* ty) {
    return (((strcmp(ty, "int") == 0) || (strcmp(ty, "float") == 0)) || (strcmp(ty, "bool") == 0));
}

const char* indent(int64_t depth) {
    const char* s = "";
    int64_t i = ((int64_t)0LL);
    while ((i < depth)) {
        (s = haki_string_concat(s, "    "));
        (i = (i + ((int64_t)1LL)));
    }
    return s;
}

const char* escapeStr(const char* s) {
    const char* out = "";
    int64_t i = ((int64_t)0LL);
    int64_t n = haki_string_length(s);
    while ((i < n)) {
        const char* ch = haki_string_substring(s, i, (i + ((int64_t)1LL)));
        if ((strcmp(ch, "\"") == 0)) {
            (out = haki_string_concat(out, "\\\""));
        }
        else {
            if ((strcmp(ch, "\\") == 0)) {
                (out = haki_string_concat(out, "\\\\"));
            }
            else {
                if ((strcmp(ch, "\n") == 0)) {
                    (out = haki_string_concat(out, "\\n"));
                }
                else {
                    if ((strcmp(ch, "\t") == 0)) {
                        (out = haki_string_concat(out, "\\t"));
                    }
                    else {
                        (out = haki_string_concat(out, ch));
                    }
                }
            }
        }
        (i = (i + ((int64_t)1LL)));
    }
    return out;
}

const char* emitExpr(compiler__Expr* e, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* tag = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int64_t n = *(int64_t*)__mpayload; __match_result = "int"; } else if (__mtag == 1LL) { int8_t b = *(int8_t*)__mpayload; __match_result = "bool"; } else if (__mtag == 2LL) { __match_result = "null"; } else if (__mtag == 3LL) { const char* s = *(const char**)__mpayload; __match_result = "string"; } else if (__mtag == 4LL) { const char* n = *(const char**)__mpayload; __match_result = "ident"; } else if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "unary"; } else if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = "binary"; } else if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = "call"; } else if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = "field"; } else if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = "method"; } else if (__mtag == 10LL) { compiler__Expr* a = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* idx = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "index"; } else if (__mtag == 11LL) { void* elems = *(void**)__mpayload; __match_result = "array"; } else if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = "if"; } else if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = "match"; } else if (__mtag == 14LL) { void* stmts = *(void**)__mpayload; __match_result = "block"; } else if (__mtag == 15LL) { compiler__Expr* inner = *(compiler__Expr**)__mpayload; __match_result = "async"; } __match_result; });
    if ((strcmp(tag, "int") == 0)) {
        int64_t n = ({ int64_t __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int64_t n = *(int64_t*)__mpayload; __match_result = n; } else { __match_result = ((int64_t)0LL); } __match_result; });
        return haki_string_concat(haki_string_concat("((int64_t)", haki_int_to_string(n)), "LL)");
    }
    if ((strcmp(tag, "bool") == 0)) {
        int8_t b = ({ int8_t __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 1LL) { int8_t b = *(int8_t*)__mpayload; __match_result = b; } else { __match_result = 0; } __match_result; });
        if (b) {
            return "1";
        }
        return "0";
    }
    if ((strcmp(tag, "null") == 0)) {
        return "NULL";
    }
    if ((strcmp(tag, "string") == 0)) {
        const char* s = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 3LL) { const char* s = *(const char**)__mpayload; __match_result = s; } else { __match_result = ""; } __match_result; });
        return haki_string_concat(haki_string_concat("\"", escapeStr(s)), "\"");
    }
    if ((strcmp(tag, "ident") == 0)) {
        const char* n = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 4LL) { const char* n = *(const char**)__mpayload; __match_result = n; } else { __match_result = ""; } __match_result; });
        return cName(n);
    }
    if ((strcmp(tag, "unary") == 0)) {
        const char* op = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = op; } else { __match_result = ""; } __match_result; });
        compiler__Expr* inner = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 5LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* inner = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = inner; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return haki_string_concat(haki_string_concat(haki_string_concat("(", op), emitExpr(inner, depth, sc, sym)), ")");
    }
    if ((strcmp(tag, "binary") == 0)) {
        const char* op = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = op; } else { __match_result = ""; } __match_result; });
        compiler__Expr* lExpr = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = l; } else { __match_result = compiler__nullExpr(); } __match_result; });
        compiler__Expr* rExpr = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { const char* op = *(const char**)((void**)__mpayload)[0]; compiler__Expr* l = *(compiler__Expr**)((void**)__mpayload)[1]; compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = r; } else { __match_result = compiler__nullExpr(); } __match_result; });
        const char* le = emitExpr(lExpr, depth, sc, sym);
        const char* re = emitExpr(rExpr, depth, sc, sym);
        const char* lty = tinfer__inferWithScope(lExpr, sc, sym);
        const char* cop = tinfer__binaryOpC(op, lty);
        if ((strcmp(cop, "string_concat") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_string_concat(", le), ", "), re), ")");
        }
        if ((strcmp(cop, "string_eq") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(strcmp(", le), ", "), re), ") == 0)");
        }
        if ((strcmp(cop, "string_neq") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(strcmp(", le), ", "), re), ") != 0)");
        }
        if ((strcmp(cop, "ptr_eq") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(", le), " == "), re), ")");
        }
        if ((strcmp(cop, "ptr_neq") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(", le), " != "), re), ")");
        }
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(", le), " "), cop), " "), re), ")");
    }
    if ((strcmp(tag, "call") == 0)) {
        const char* n = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = n; } else { __match_result = ""; } __match_result; });
        void* args = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = args; } else { __match_result = compiler__emptyExprs(); } __match_result; });
        return emitCall(n, args, depth, sc, sym);
    }
    if ((strcmp(tag, "field") == 0)) {
        compiler__Expr* recv = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = recv; } else { __match_result = compiler__nullExpr(); } __match_result; });
        const char* f = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* recv = *(compiler__Expr**)((void**)__mpayload)[0]; const char* f = *(const char**)((void**)__mpayload)[1]; __match_result = f; } else { __match_result = ""; } __match_result; });
        if ((strcmp(f, "message") == 0)) {
            return haki_string_concat(haki_string_concat("haki_error_message(", emitExpr(recv, depth, sc, sym)), ")");
        }
        if ((strcmp(f, "cause") == 0)) {
            return haki_string_concat(haki_string_concat("haki_error_cause(", emitExpr(recv, depth, sc, sym)), ")");
        }
        if ((strcmp(f, "length") == 0)) {
            return haki_string_concat(haki_string_concat("haki_array_length(", emitExpr(recv, depth, sc, sym)), ")");
        }
        return haki_string_concat(haki_string_concat(emitExpr(recv, depth, sc, sym), "->"), cName(f));
    }
    if ((strcmp(tag, "method") == 0)) {
        compiler__Expr* recv = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = r; } else { __match_result = compiler__nullExpr(); } __match_result; });
        const char* m = ({ const char* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = m; } else { __match_result = ""; } __match_result; });
        void* args = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { compiler__Expr* r = *(compiler__Expr**)((void**)__mpayload)[0]; const char* m = *(const char**)((void**)__mpayload)[1]; void* ma = *(void**)((void**)__mpayload)[2]; __match_result = ma; } else { __match_result = compiler__emptyExprs(); } __match_result; });
        const char* re = emitExpr(recv, depth, sc, sym);
        if (((strcmp(m, "append") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
            const char* av = emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym);
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("({ void* __el = ", av), "; haki_array_append("), re), ", &__el); })");
        }
        if ((strcmp(m, "length") == 0)) {
            return haki_string_concat(haki_string_concat("haki_array_length(", re), ")");
        }
        if ((strcmp(m, "has") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_map_has(", re), ", "), emitExprList(args, depth, sc, sym)), ")");
        }
        if ((strcmp(m, "get") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_map_get(", re), ", "), emitExprList(args, depth, sc, sym)), ")");
        }
        if ((strcmp(m, "set") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_map_set(", re), ", "), emitExprList(args, depth, sc, sym)), ")");
        }
        if ((strcmp(m, "substring") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_string_substring(", re), ", "), emitExprList(args, depth, sc, sym)), ")");
        }
        if ((strcmp(m, "contains") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_string_contains(", re), ", "), emitExprList(args, depth, sc, sym)), ")");
        }
        if ((strcmp(m, "split") == 0)) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_string_split(", re), ", "), emitExprList(args, depth, sc, sym)), ")");
        }
        if ((strcmp(m, "toUpper") == 0)) {
            return haki_string_concat(haki_string_concat("haki_string_to_upper(", re), ")");
        }
        if ((strcmp(m, "toLower") == 0)) {
            return haki_string_concat(haki_string_concat("haki_string_to_lower(", re), ")");
        }
        if ((strcmp(m, "trim") == 0)) {
            return haki_string_concat(haki_string_concat("haki_string_trim(", re), ")");
        }
        const char* argStr = "";
        if ((haki_array_length(args) > ((int64_t)0LL))) {
            (argStr = haki_string_concat(", ", emitExprList(args, depth, sc, sym)));
        }
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(re, "->"), cName(m)), "("), re), argStr), ")");
    }
    if ((strcmp(tag, "index") == 0)) {
        compiler__Expr* arr = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 10LL) { compiler__Expr* a = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* idx = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = a; } else { __match_result = compiler__nullExpr(); } __match_result; });
        compiler__Expr* idx = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 10LL) { compiler__Expr* a = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* idx = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = idx; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("(*((void**)haki_array_get(", emitExpr(arr, depth, sc, sym)), ", "), emitExpr(idx, depth, sc, sym)), ")))");
    }
    if ((strcmp(tag, "array") == 0)) {
        return "haki_array_new(sizeof(void*))";
    }
    if ((strcmp(tag, "if") == 0)) {
        compiler__Expr* cond = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = c; } else { __match_result = compiler__nullExpr(); } __match_result; });
        void* then = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = th; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        void* els = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 12LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = el; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        const char* ce = emitExpr(cond, depth, sc, sym);
        const char* tv = yieldVal(then, depth, sc, sym);
        const char* ev = yieldVal(els, depth, sc, sym);
        if (((haki_string_length(tv) > ((int64_t)0LL)) && (haki_string_length(ev) > ((int64_t)0LL)))) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("((", ce), ") ? ("), tv), ") : ("), ev), "))");
        }
        return ce;
    }
    if ((strcmp(tag, "match") == 0)) {
        compiler__Expr* scrut = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = s; } else { __match_result = compiler__nullExpr(); } __match_result; });
        void* arms = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 13LL) { compiler__Expr* s = *(compiler__Expr**)((void**)__mpayload)[0]; void* arms = *(void**)((void**)__mpayload)[1]; __match_result = arms; } else { __match_result = compiler__emptyArms(); } __match_result; });
        return emitMatchExpr(scrut, arms, depth, sc, sym);
    }
    if ((strcmp(tag, "block") == 0)) {
        void* stmts = ({ void* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 14LL) { void* stmts = *(void**)__mpayload; __match_result = stmts; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        const char* yv = yieldVal(stmts, depth, sc, sym);
        if ((haki_string_length(yv) > ((int64_t)0LL))) {
            return haki_string_concat(haki_string_concat("({ ", yv), "; })");
        }
        return "0";
    }
    if ((strcmp(tag, "async") == 0)) {
        compiler__Expr* inner = ({ compiler__Expr* __match_result;  void* __msc = (void*)e;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 15LL) { compiler__Expr* inner = *(compiler__Expr**)__mpayload; __match_result = inner; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return haki_string_concat(haki_string_concat("haki_task_spawn_simple((void(*)(void*))", emitExpr(inner, depth, sc, sym)), ", NULL)");
    }
    return "/* unknown expr */";
}

const char* emitExprList(void* args, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* s = "";
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(args))) {
        if ((i > ((int64_t)0LL))) {
            (s = haki_string_concat(s, ", "));
        }
        (s = haki_string_concat(s, emitExpr((*(compiler__Expr**)haki_array_get(args, i)), depth, sc, sym)));
        (i = (i + ((int64_t)1LL)));
    }
    return s;
}

const char* yieldVal(void* stmts, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    { void* __arr_stmt = stmts;
        int64_t __len_stmt = haki_array_length(__arr_stmt);
        for (int64_t __i_stmt = 0; __i_stmt < __len_stmt; __i_stmt++) {
            compiler__Stmt* stmt = *(compiler__Stmt**)haki_array_get(__arr_stmt, __i_stmt);
            const char* tag = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 2LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = "yield"; } else { __match_result = ""; } __match_result; });
            if ((strcmp(tag, "yield") == 0)) {
                compiler__Expr* e = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 2LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = e; } else { __match_result = compiler__nullExpr(); } __match_result; });
                return emitExpr(e, depth, sc, sym);
            }
        }
    }
    return "";
}

const char* emitMatchExpr(compiler__Expr* scrut, void* arms, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* se = emitExpr(scrut, depth, sc, sym);
    const char* parts = haki_string_concat(haki_string_concat("void* __msc = (void*)(", se), "); int64_t __mtag = ((int64_t*)__msc)[0]; void* __mpayload = ((void**)__msc)[1]; ");
    (parts = haki_string_concat(parts, "int64_t __mr; "));
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(arms))) {
        compiler__MatchArm* arm = (*(compiler__MatchArm**)haki_array_get(arms, i));
        const char* prefix = "";
        if ((i == ((int64_t)0LL))) {
            (prefix = "if");
        }
        else {
            (prefix = "} else if");
        }
        if ((strcmp(arm->pattern, "_") == 0)) {
            (parts = haki_string_concat(parts, "} else { "));
        }
        else {
            (parts = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(parts, prefix), " (__mtag == "), haki_int_to_string(i)), "LL) { "));
        }
        int64_t bi = ((int64_t)0LL);
        while ((bi < haki_array_length(arm->bindings))) {
            (parts = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(parts, "void* "), cName((*(const char**)haki_array_get(arm->bindings, bi)))), " = ((void**)__mpayload)["), haki_int_to_string(bi)), "]; "));
            (bi = (bi + ((int64_t)1LL)));
        }
        const char* yv = yieldVal(arm->body, depth, sc, sym);
        if ((haki_string_length(yv) > ((int64_t)0LL))) {
            (parts = haki_string_concat(haki_string_concat(haki_string_concat(parts, "__mr = (int64_t)("), yv), "); "));
        }
        (i = (i + ((int64_t)1LL)));
    }
    (parts = haki_string_concat(parts, "} __mr;"));
    return haki_string_concat(haki_string_concat("({ ", parts), "})");
}

const char* emitCall(const char* name, void* args, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    if (((strcmp(name, "print") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_print(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if (((strcmp(name, "print_int") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_print_int(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if (((strcmp(name, "print_bool") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_print_bool(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if (((strcmp(name, "print_float") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_print_float(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if (((strcmp(name, "int_to_string") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_int_to_string(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if (((strcmp(name, "string_length") == 0) && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_string_length(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if ((strcmp(name, "panic") == 0)) {
        return haki_string_concat(haki_string_concat("haki_panic(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if ((strcmp(name, "argv") == 0)) {
        return "haki_argv()";
    }
    if ((strcmp(name, "readFile") == 0)) {
        return haki_string_concat(haki_string_concat("haki_read_file(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if ((strcmp(name, "fileExists") == 0)) {
        return haki_string_concat(haki_string_concat("haki_file_exists(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if ((strcmp(name, "Error") == 0)) {
        if ((haki_array_length(args) == ((int64_t)1LL))) {
            return haki_string_concat(haki_string_concat("haki_error_new(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
        }
    }
    if ((haki_string_contains(name, "__append") && (haki_array_length(args) == ((int64_t)2LL)))) {
        const char* av = emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)1LL))), depth, sc, sym);
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("({ void* __el = (void*)(", av), "); haki_array_append("), emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ", &__el); })");
    }
    if ((haki_string_contains(name, "__length") && (haki_array_length(args) == ((int64_t)1LL)))) {
        return haki_string_concat(haki_string_concat("haki_array_length(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ")");
    }
    if ((haki_string_contains(name, "__has") && (haki_array_length(args) == ((int64_t)2LL)))) {
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_map_has(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ", "), emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)1LL))), depth, sc, sym)), ")");
    }
    if ((haki_string_contains(name, "__set") && (haki_array_length(args) == ((int64_t)3LL)))) {
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_map_set(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ", "), emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)1LL))), depth, sc, sym)), ", "), emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)2LL))), depth, sc, sym)), ")");
    }
    if ((haki_string_contains(name, "__getOrDefault") && (haki_array_length(args) == ((int64_t)3LL)))) {
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat("haki_map_get_or_default(", emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)0LL))), depth, sc, sym)), ", "), emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)1LL))), depth, sc, sym)), ", "), emitExpr((*(compiler__Expr**)haki_array_get(args, ((int64_t)2LL))), depth, sc, sym)), ")");
    }
    const char* argStr = "";
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(args))) {
        if ((i > ((int64_t)0LL))) {
            (argStr = haki_string_concat(argStr, ", "));
        }
        (argStr = haki_string_concat(argStr, emitExpr((*(compiler__Expr**)haki_array_get(args, i)), depth, sc, sym)));
        (i = (i + ((int64_t)1LL)));
    }
    return haki_string_concat(haki_string_concat(haki_string_concat(cName(name), "("), argStr), ")");
}

const char* emitStmt(compiler__Stmt* stmt, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* ind = indent(depth);
    const char* tag = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = "let"; } else if (__mtag == 1LL) { void* vals = *(void**)__mpayload; __match_result = "return"; } else if (__mtag == 2LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = "yield"; } else if (__mtag == 3LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = "defer"; } else if (__mtag == 4LL) { __match_result = "continue"; } else if (__mtag == 5LL) { __match_result = "break"; } else if (__mtag == 6LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = "expr"; } else if (__mtag == 10LL) { compiler__Expr* target = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* val = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = "assign"; } else if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = "if"; } else if (__mtag == 8LL) { compiler__Expr* cond = *(compiler__Expr**)((void**)__mpayload)[0]; void* body = *(void**)((void**)__mpayload)[1]; __match_result = "while"; } else if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = "for"; } __match_result; });
    if ((strcmp(tag, "let") == 0)) {
        int8_t isMut = ({ int8_t __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = isMut; } else { __match_result = 0; } __match_result; });
        const char* name = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = name; } else { __match_result = ""; } __match_result; });
        compiler__Expr* init = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 0LL) { int8_t isMut = *(int8_t*)((void**)__mpayload)[0]; const char* name = *(const char**)((void**)__mpayload)[1]; compiler__Expr* init = *(compiler__Expr**)((void**)__mpayload)[2]; __match_result = init; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return emitLet(name, init, depth, sc, sym);
    }
    if ((strcmp(tag, "return") == 0)) {
        void* vals = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 1LL) { void* vals = *(void**)__mpayload; __match_result = vals; } else { __match_result = compiler__emptyExprs(); } __match_result; });
        if ((haki_array_length(vals) == ((int64_t)0LL))) {
            return haki_string_concat(ind, "return;\n");
        }
        if ((haki_array_length(vals) == ((int64_t)1LL))) {
            return haki_string_concat(haki_string_concat(haki_string_concat(ind, "return "), emitExpr((*(compiler__Expr**)haki_array_get(vals, ((int64_t)0LL))), depth, sc, sym)), ";\n");
        }
        const char* n = haki_int_to_string(haki_array_length(vals));
        const char* s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(ind, "__Tuple"), n), "* __ret = (__Tuple"), n), "*)malloc(sizeof(__Tuple"), n), "));\n");
        int64_t i = ((int64_t)0LL);
        while ((i < haki_array_length(vals))) {
            const char* ve = emitExpr((*(compiler__Expr**)haki_array_get(vals, i)), depth, sc, sym);
            const char* si = haki_int_to_string(i);
            if (isScalarTy("int")) {
                (s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(s, ind), "{ int64_t* __f = malloc(sizeof(int64_t)); *__f = "), ve), "; __ret->f"), si), " = __f; }\n"));
            }
            else {
                (s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(s, ind), "__ret->f"), si), " = (void*)("), ve), ");\n"));
            }
            (i = (i + ((int64_t)1LL)));
        }
        (s = haki_string_concat(haki_string_concat(s, ind), "return __ret;\n"));
        return s;
    }
    if ((strcmp(tag, "yield") == 0)) {
        return "/* yield â handled by parent */\n";
    }
    if ((strcmp(tag, "defer") == 0)) {
        return "/* defer â handled by parent */\n";
    }
    if ((strcmp(tag, "continue") == 0)) {
        return haki_string_concat(ind, "continue;\n");
    }
    if ((strcmp(tag, "break") == 0)) {
        return haki_string_concat(ind, "break;\n");
    }
    if ((strcmp(tag, "expr") == 0)) {
        compiler__Expr* e = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 6LL) { compiler__Expr* e = *(compiler__Expr**)__mpayload; __match_result = e; } else { __match_result = compiler__nullExpr(); } __match_result; });
        const char* es = emitExpr(e, depth, sc, sym);
        if ((haki_string_length(es) > ((int64_t)0LL))) {
            return haki_string_concat(haki_string_concat(ind, es), ";\n");
        }
        return "";
    }
    if ((strcmp(tag, "assign") == 0)) {
        compiler__Expr* target = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 10LL) { compiler__Expr* t = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* v = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = t; } else { __match_result = compiler__nullExpr(); } __match_result; });
        compiler__Expr* val = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 10LL) { compiler__Expr* t = *(compiler__Expr**)((void**)__mpayload)[0]; compiler__Expr* v = *(compiler__Expr**)((void**)__mpayload)[1]; __match_result = v; } else { __match_result = compiler__nullExpr(); } __match_result; });
        return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(ind, emitExpr(target, depth, sc, sym)), " = "), emitExpr(val, depth, sc, sym)), ";\n");
    }
    if ((strcmp(tag, "if") == 0)) {
        compiler__Expr* cond = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = c; } else { __match_result = compiler__nullExpr(); } __match_result; });
        void* then = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = th; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        void* els = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* th = *(void**)((void**)__mpayload)[1]; void* el = *(void**)((void**)__mpayload)[2]; __match_result = el; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        const char* s = haki_string_concat(haki_string_concat(haki_string_concat(ind, "if ("), emitExpr(cond, depth, sc, sym)), ") {\n");
        { void* __arr_st = then;
            int64_t __len_st = haki_array_length(__arr_st);
            for (int64_t __i_st = 0; __i_st < __len_st; __i_st++) {
                compiler__Stmt* st = *(compiler__Stmt**)haki_array_get(__arr_st, __i_st);
                (s = haki_string_concat(s, emitStmt(st, (depth + ((int64_t)1LL)), sc, sym)));
            }
        }
        (s = haki_string_concat(haki_string_concat(s, ind), "}\n"));
        if ((haki_array_length(els) > ((int64_t)0LL))) {
            (s = haki_string_concat(haki_string_concat(s, ind), "else {\n"));
            { void* __arr_st = els;
                int64_t __len_st = haki_array_length(__arr_st);
                for (int64_t __i_st = 0; __i_st < __len_st; __i_st++) {
                    compiler__Stmt* st = *(compiler__Stmt**)haki_array_get(__arr_st, __i_st);
                    (s = haki_string_concat(s, emitStmt(st, (depth + ((int64_t)1LL)), sc, sym)));
                }
            }
            (s = haki_string_concat(haki_string_concat(s, ind), "}\n"));
        }
        return s;
    }
    if ((strcmp(tag, "while") == 0)) {
        compiler__Expr* cond = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* body = *(void**)((void**)__mpayload)[1]; __match_result = c; } else { __match_result = compiler__nullExpr(); } __match_result; });
        void* body = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 8LL) { compiler__Expr* c = *(compiler__Expr**)((void**)__mpayload)[0]; void* body = *(void**)((void**)__mpayload)[1]; __match_result = body; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        const char* s = haki_string_concat(haki_string_concat(haki_string_concat(ind, "while ("), emitExpr(cond, depth, sc, sym)), ") {\n");
        { void* __arr_st = body;
            int64_t __len_st = haki_array_length(__arr_st);
            for (int64_t __i_st = 0; __i_st < __len_st; __i_st++) {
                compiler__Stmt* st = *(compiler__Stmt**)haki_array_get(__arr_st, __i_st);
                (s = haki_string_concat(s, emitStmt(st, (depth + ((int64_t)1LL)), sc, sym)));
            }
        }
        (s = haki_string_concat(haki_string_concat(s, ind), "}\n"));
        return s;
    }
    if ((strcmp(tag, "for") == 0)) {
        const char* varName = ({ const char* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = v; } else { __match_result = "__it"; } __match_result; });
        compiler__Expr* iter = ({ compiler__Expr* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = iter; } else { __match_result = compiler__nullExpr(); } __match_result; });
        void* body = ({ void* __match_result;  void* __msc = (void*)stmt;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 9LL) { const char* v = *(const char**)((void**)__mpayload)[0]; compiler__Expr* iter = *(compiler__Expr**)((void**)__mpayload)[1]; void* body = *(void**)((void**)__mpayload)[2]; __match_result = body; } else { __match_result = compiler__emptyStmts(); } __match_result; });
        const char* arrVar = haki_string_concat("__arr_", cName(varName));
        const char* idxVar = haki_string_concat("__i_", cName(varName));
        const char* s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(ind, "{ void* "), arrVar), " = "), emitExpr(iter, depth, sc, sym)), ";\n");
        (s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(s, ind), "  int64_t __len_"), cName(varName)), " = haki_array_length("), arrVar), ");\n"));
        (s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(s, ind), "  for (int64_t "), idxVar), " = 0; "), idxVar), " < __len_"), cName(varName)), "; "), idxVar), "++) {\n"));
        (s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(s, indent((depth + ((int64_t)2LL)))), "void* "), cName(varName)), " = *(void**)haki_array_get("), arrVar), ", "), idxVar), ");\n"));
        { void* __arr_st = body;
            int64_t __len_st = haki_array_length(__arr_st);
            for (int64_t __i_st = 0; __i_st < __len_st; __i_st++) {
                compiler__Stmt* st = *(compiler__Stmt**)haki_array_get(__arr_st, __i_st);
                (s = haki_string_concat(s, emitStmt(st, (depth + ((int64_t)2LL)), sc, sym)));
            }
        }
        (s = haki_string_concat(haki_string_concat(s, ind), "  }\n"));
        (s = haki_string_concat(haki_string_concat(s, ind), "}\n"));
        return s;
    }
    return haki_string_concat(ind, "/* unknown stmt */\n");
}

const char* emitLet(const char* name, compiler__Expr* init, int64_t depth, tinfer__Scope* sc, typeck__SymTable* sym) {
    const char* ind = indent(depth);
    const char* nm = cName(name);
    if ((strcmp(nm, "_") == 0)) {
        return haki_string_concat(haki_string_concat(haki_string_concat(ind, "(void)("), emitExpr(init, depth, sc, sym)), ");\n");
    }
    int8_t isArr = ({ int8_t __match_result;  void* __msc = (void*)init;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 11LL) { void* elems = *(void**)__mpayload; __match_result = 1; } else { __match_result = 0; } __match_result; });
    if (isArr) {
        void* elems = ({ void* __match_result;  void* __msc = (void*)init;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 11LL) { void* elems = *(void**)__mpayload; __match_result = elems; } else { __match_result = compiler__emptyExprs(); } __match_result; });
        const char* s = haki_string_concat(haki_string_concat(haki_string_concat(ind, "void* "), nm), " = haki_array_new(sizeof(void*));\n");
        { void* __arr_el = elems;
            int64_t __len_el = haki_array_length(__arr_el);
            for (int64_t __i_el = 0; __i_el < __len_el; __i_el++) {
                compiler__Expr* el = *(compiler__Expr**)haki_array_get(__arr_el, __i_el);
                const char* ev = emitExpr(el, depth, sc, sym);
                (s = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(s, ind), "{ void* __el = (void*)("), ev), "); haki_array_append("), nm), ", &__el); }\n"));
            }
        }
        return s;
    }
    int8_t isCall = ({ int8_t __match_result;  void* __msc = (void*)init;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = 1; } else { __match_result = 0; } __match_result; });
    if (isCall) {
        const char* callName = ({ const char* __match_result;  void* __msc = (void*)init;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = n; } else { __match_result = ""; } __match_result; });
        void* callArgs = ({ void* __match_result;  void* __msc = (void*)init;  int64_t __mtag = ((int64_t*)__msc)[0];  void* __mpayload = ((void**)__msc)[1];  if (__mtag == 7LL) { const char* n = *(const char**)((void**)__mpayload)[0]; void* args = *(void**)((void**)__mpayload)[1]; __match_result = args; } else { __match_result = compiler__emptyExprs(); } __match_result; });
        if (((strcmp(callName, "Error") == 0) && (haki_array_length(callArgs) == ((int64_t)1LL)))) {
            return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(ind, "void* "), nm), " = haki_error_new("), emitExpr((*(compiler__Expr**)haki_array_get(callArgs, ((int64_t)0LL))), depth, sc, sym)), ");\n");
        }
    }
    const char* initStr = emitExpr(init, depth, sc, sym);
    return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(ind, "void* "), nm), " = (void*)("), initStr), ");\n");
}

const char* emitFnProto(mono__MonoFn* f) {
    const char* ret = cRetTy(f->retTy);
    const char* params = "";
    int64_t i = ((int64_t)0LL);
    while ((i < haki_array_length(f->params))) {
        if ((i > ((int64_t)0LL))) {
            (params = haki_string_concat(params, ", "));
        }
        (params = haki_string_concat(haki_string_concat(haki_string_concat(params, cTy((*(compiler__Param**)haki_array_get(f->params, i))->ty)), " "), cName((*(compiler__Param**)haki_array_get(f->params, i))->name)));
        (i = (i + ((int64_t)1LL)));
    }
    if ((strcmp(f->name, "main") == 0)) {
        return "int main(int argc, char** argv)";
    }
    if ((haki_string_length(params) == ((int64_t)0LL))) {
        (params = "void");
    }
    return haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(ret, " "), cName(f->name)), "("), params), ")");
}

const char* emitFn(mono__MonoFn* f, typeck__SymTable* sym) {
    compiler__FnDef* fnDef = compiler__makeFnDef(f->name, f->params, f->retTy, f->body);
    tinfer__Scope* sc = tinfer__buildFnScope(fnDef, sym);
    const char* s = haki_string_concat(emitFnProto(f), " {\n");
    if ((strcmp(f->name, "main") == 0)) {
        (s = haki_string_concat(s, "    haki_runtime_init(argc, argv);\n"));
    }
    { void* __arr_stmt = f->body;
        int64_t __len_stmt = haki_array_length(__arr_stmt);
        for (int64_t __i_stmt = 0; __i_stmt < __len_stmt; __i_stmt++) {
            compiler__Stmt* stmt = *(compiler__Stmt**)haki_array_get(__arr_stmt, __i_stmt);
            (s = haki_string_concat(s, emitStmt(stmt, ((int64_t)1LL), sc, sym)));
        }
    }
    if (((strcmp(f->retTy, "void") == 0) || (strcmp(f->retTy, "") == 0))) {
        if ((strcmp(f->name, "main") == 0)) {
            (s = haki_string_concat(s, "    return 0;\n"));
        }
    }
    (s = haki_string_concat(s, "}\n\n"));
    return s;
}

const char* emitStructDef(compiler__StructDef* s) {
    if ((haki_string_length(s->name) == ((int64_t)0LL))) {
        return "";
    }
    const char* out = haki_string_concat(haki_string_concat("struct ", cName(s->name)), " {\n");
    { void* __arr_f = s->fields;
        int64_t __len_f = haki_array_length(__arr_f);
        for (int64_t __i_f = 0; __i_f < __len_f; __i_f++) {
            compiler__Param* f = *(compiler__Param**)haki_array_get(__arr_f, __i_f);
            (out = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(out, "    "), cTy(f->ty)), " "), cName(f->name)), ";\n"));
        }
    }
    if ((haki_array_length(s->fields) == ((int64_t)0LL))) {
        (out = haki_string_concat(out, "    int _dummy;\n"));
    }
    (out = haki_string_concat(out, "};\n"));
    return out;
}

const char* tupleStructs(void) {
    const char* s = "/* Tuple structs for multi-return */\n";
    int64_t n = ((int64_t)2LL);
    while ((n <= ((int64_t)4LL))) {
        (s = haki_string_concat(s, "typedef struct { "));
        int64_t i = ((int64_t)0LL);
        while ((i < n)) {
            (s = haki_string_concat(haki_string_concat(haki_string_concat(s, "void* f"), haki_int_to_string(i)), "; "));
            (i = (i + ((int64_t)1LL)));
        }
        (s = haki_string_concat(haki_string_concat(haki_string_concat(s, "} __Tuple"), haki_int_to_string(n)), ";\n"));
        (n = (n + ((int64_t)1LL)));
    }
    return haki_string_concat(s, "\n");
}

const char* emitProgram(mono__MonoProgram* prog, const char* runtimeSrc) {
    typeck__SymTable* sym = typeck__symNew();
    const char* out = "/* Generated by hakic --emit-c (Haki bootstrap emitter) */\n";
    (out = haki_string_concat(out, "/* Compile: gcc -std=gnu11 -O2 -lpthread -lm -o out this.c */\n\n"));
    (out = haki_string_concat(haki_string_concat(out, runtimeSrc), "\n"));
    (out = haki_string_concat(out, tupleStructs()));
    (out = haki_string_concat(out, "/* Forward declarations */\n"));
    { void* __arr_s = prog->structs;
        int64_t __len_s = haki_array_length(__arr_s);
        for (int64_t __i_s = 0; __i_s < __len_s; __i_s++) {
            compiler__StructDef* s = *(compiler__StructDef**)haki_array_get(__arr_s, __i_s);
            if ((haki_string_length(s->name) > ((int64_t)0LL))) {
                (out = haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(haki_string_concat(out, "typedef struct "), cName(s->name)), " "), cName(s->name)), ";\n"));
            }
        }
    }
    (out = haki_string_concat(out, "typedef struct mono__MonoProgram mono__MonoProgram;\n"));
    (out = haki_string_concat(out, "typedef struct mono__MonoFn mono__MonoFn;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__FnDef compiler__FnDef;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__StructDef compiler__StructDef;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__Param compiler__Param;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__Item compiler__Item;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__Expr compiler__Expr;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__Stmt compiler__Stmt;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__MatchArm compiler__MatchArm;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__Token compiler__Token;\n"));
    (out = haki_string_concat(out, "typedef struct typeck__SymTable typeck__SymTable;\n"));
    (out = haki_string_concat(out, "typedef struct typeck__FnInfo typeck__FnInfo;\n"));
    (out = haki_string_concat(out, "typedef struct tinfer__Scope tinfer__Scope;\n"));
    (out = haki_string_concat(out, "typedef struct tinfer__TypedExpr tinfer__TypedExpr;\n"));
    (out = haki_string_concat(out, "typedef struct mono__MonoFn mono__MonoFn;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__FnDef compiler__FnDef;\n"));
    (out = haki_string_concat(out, "typedef struct compiler__StructDef compiler__StructDef;\n"));
    (out = haki_string_concat(out, "typedef struct typeck__SymTable typeck__SymTable;\n"));
    (out = haki_string_concat(out, "typedef struct tinfer__Scope tinfer__Scope;\n"));
    (out = haki_string_concat(out, "\n"));
    (out = haki_string_concat(out, "/* Function prototypes */\n"));
    { void* __arr_f = prog->fns;
        int64_t __len_f = haki_array_length(__arr_f);
        for (int64_t __i_f = 0; __i_f < __len_f; __i_f++) {
            mono__MonoFn* f = *(mono__MonoFn**)haki_array_get(__arr_f, __i_f);
            (out = haki_string_concat(haki_string_concat(out, emitFnProto(f)), ";\n"));
        }
    }
    (out = haki_string_concat(out, "\n"));
    (out = haki_string_concat(out, "/* Struct definitions */\n"));
    { void* __arr_s = prog->structs;
        int64_t __len_s = haki_array_length(__arr_s);
        for (int64_t __i_s = 0; __i_s < __len_s; __i_s++) {
            compiler__StructDef* s = *(compiler__StructDef**)haki_array_get(__arr_s, __i_s);
            (out = haki_string_concat(out, emitStructDef(s)));
        }
    }
    (out = haki_string_concat(out, "\n"));
    (out = haki_string_concat(out, "/* Functions */\n"));
    { void* __arr_f = prog->fns;
        int64_t __len_f = haki_array_length(__arr_f);
        for (int64_t __i_f = 0; __i_f < __len_f; __i_f++) {
            mono__MonoFn* f = *(mono__MonoFn**)haki_array_get(__arr_f, __i_f);
            (out = haki_string_concat(out, emitFn(f, sym)));
        }
    }
    return out;
}

void* compileToC(const char* src, const char* runtimeSrc) {
    __Tuple2* __mb_29643 = (__Tuple2*)(mono__monoFromSource(src));
    mono__MonoProgram* prog = (mono__MonoProgram*)__mb_29643->f0;
    void* err = (void*)__mb_29643->f1;
    if ((err != NULL)) {
        __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
        __ret->f0 = (void*)("");
        __ret->f1 = (void*)(err);
        return __ret;
    }
    __Tuple2* __ret = (__Tuple2*)malloc(sizeof(__Tuple2));
    __ret->f0 = (void*)(emitProgram(prog, runtimeSrc));
    __ret->f1 = (void*)(NULL);
    return __ret;
}

void test_c_name(void) {
    if ((!(strcmp(cName("int"), "haki_int") == 0))) {
        haki_panic("cName int wrong");
    }
    if ((!(strcmp(cName("foo"), "foo") == 0))) {
        haki_panic("cName foo wrong");
    }
    if ((!(strcmp(cName("add"), "add") == 0))) {
        haki_panic("cName add wrong");
    }
}

void test_c_ty(void) {
    if ((!(strcmp(cTy("int"), "int64_t") == 0))) {
        haki_panic("cTy int wrong");
    }
    if ((!(strcmp(cTy("string"), "const char*") == 0))) {
        haki_panic("cTy string wrong");
    }
    if ((!(strcmp(cTy("bool"), "int8_t") == 0))) {
        haki_panic("cTy bool wrong");
    }
    if ((!(strcmp(cTy("void"), "void") == 0))) {
        haki_panic("cTy void wrong");
    }
}

void test_escape_str(void) {
    const char* s = escapeStr("hello\nworld");
    if ((!(strcmp(s, "hello\\nworld") == 0))) {
        haki_panic(haki_string_concat("escapeStr wrong: ", s));
    }
}

void test_emit_proto(void) {
    void* params = haki_array_new(sizeof(void*));
    void* body = haki_array_new(sizeof(void*));
    mono__MonoFn* mf = mono__makeMonoFn("add", params, "int", body);
    const char* proto = emitFnProto(mf);
    if ((!(strcmp(proto, "int64_t add(void)") == 0))) {
        haki_panic(haki_string_concat("proto wrong: ", proto));
    }
}

int main(int argc, char** argv) {
    haki_runtime_init(argc, argv);
    const char* miniSrc = "fn add(a: int, b: int) -> int { return a + b }\nfn main() { print_int(add(1, 2)) }";
    __Tuple2* __mb_31023 = (__Tuple2*)(compileToC(miniSrc, "/* runtime stub */"));
    const char* c = (const char*)__mb_31023->f0;
    void* err = (void*)__mb_31023->f1;
    if ((err != NULL)) {
        haki_print(haki_string_concat("error: ", haki_error_message(err)));
        return;
    }
    haki_print(haki_string_concat(haki_string_concat("emitted ", haki_int_to_string(haki_string_length(c))), " bytes of C"));
    haki_print("done");
    return 0;
}

