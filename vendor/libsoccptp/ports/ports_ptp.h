/* Copyright 2015 Sony Corporation */
/* Sony Confidential               */

#ifndef __PORTS_PTP_H__
#define __PORTS_PTP_H__

#include <stdint.h>

namespace com {
namespace sony {
namespace imaging {
namespace ports {

class ports_ptp{
public :
    virtual ~ports_ptp() {};
    virtual socc_error send(uint16_t code, uint32_t* parameters, uint8_t num, com::sony::imaging::remote::Container& response, void* data, uint32_t size) = 0;
    virtual socc_error receive(uint16_t code, uint32_t* parameters, uint8_t num, com::sony::imaging::remote::Container& response, void** data, uint32_t& size) = 0;
    virtual socc_error wait_event(com::sony::imaging::remote::Container& container) = 0;
    virtual void dispose_data(void** data) = 0;
};

} } } }
#endif
