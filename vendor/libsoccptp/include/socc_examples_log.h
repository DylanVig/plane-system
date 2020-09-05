/* Copyright 2015 Sony Corporation */
/* Sony Confidential               */

#ifndef __SOCC_EXAMPLES_LOG_H__
#define __SOCC_EXAMPLES_LOG_H__

#include <stdio.h>
#include <stdarg.h>

class socc_examples_log {
public :
    typedef enum {
        SOCC_EXAMPLES_LOG_NONE     = 0x0,
        SOCC_EXAMPLES_LOG_ERROR    = 0x1,
        SOCC_EXAMPLES_LOG_WARN     = 0x2,
        SOCC_EXAMPLES_LOG_INFO     = 0x4,
        SOCC_EXAMPLES_LOG_VERBOSE  = 0x8
    } socc_excamples_log_levelt_t;
    
    socc_examples_log(const char* who, socc_excamples_log_levelt_t level=SOCC_EXAMPLES_LOG_WARN)
    :who(who), level(level){
    }
    
    void e(const char* fmt, ...){
        if(level < SOCC_EXAMPLES_LOG_ERROR){
            return;
        }
        va_list va;
        va_start(va, fmt);
        vprintf("\x1b[31me;", va);
        vprintf(who,va);
        vprintf(";",va);
        vprintf(fmt, va);
        vprintf("\x1b[39m\n", va);
        va_end(va);
    }
    void w(const char* fmt, ...){
        if(level < SOCC_EXAMPLES_LOG_WARN){
            return;
        }
        va_list va;
        va_start(va, fmt);
        vprintf("\x1b[35mw;", va);
        vprintf(who,va);
        vprintf(";",va);
        vprintf(fmt, va);
        vprintf("\x1b[39m\n", va);
        va_end(va);
    }
    void i(const char* fmt, ...){
        if(level < SOCC_EXAMPLES_LOG_INFO){
            return;
        }
        va_list va;
        va_start(va, fmt);
        vprintf("\x1b[32mi;", va);
        vprintf(who,va);
        vprintf(";",va);
        vprintf(fmt, va);
        vprintf("\x1b[39m\n", va);
        va_end(va);
    }
    void v(const char* fmt, ...){
        if(level < SOCC_EXAMPLES_LOG_VERBOSE){
            return;
        }
        va_list va;
        va_start(va, fmt);
        vprintf("\x1b[39mv;", va);
        vprintf(who,va);
        vprintf(";",va);
        vprintf(fmt, va);
        vprintf("\x1b[39m\n", va);
        va_end(va);
    }
    int printf(const char* fmt, ...){ /* message	*/
        int ret = 0;
        va_list va;
        va_start(va, fmt);
        ret += vprintf("\x1b[32m", va);
        ret += vprintf(fmt, va);
        ret += vprintf("\x1b[39m", va);
        va_end(va);
        return ret;
    }

    template <typename T>
    void assert_socc(const char* what, T expect, T actual){
        if(expect == actual){
            if(level >= SOCC_EXAMPLES_LOG_VERBOSE){
            fprintf(stderr, "v;%s;%s;OK;expect=%x(%d);actual=%x(%d)\n\x1b[39m", who, what, expect, expect, actual, actual);
            }
        } else {
            if(level >= SOCC_EXAMPLES_LOG_ERROR){
                fprintf(stderr, "\x1b[31me;%s;%s;NG;expect=%x(%d);actual=%x(%d)\n\x1b[39m", who, what, expect, expect, actual, actual);
            }
            fprintf(stderr, "\x1b[31mPower off the camera or disconnect USB cable before next operations.\n\x1b[39m");
            exit(EXIT_FAILURE);
        }
    }

private :

    const char* who;
    socc_excamples_log_levelt_t level;

};

#endif
