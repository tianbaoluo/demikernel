/*
 * Copyright (c) Microsoft Corporation.
 * Licensed under the MIT license.
 */

#include <rte_config.h>
#include <rte_errno.h>
#include <rte_ethdev.h>
#include <rte_ether.h>
#include <rte_mbuf.h>

void rte_pktmbuf_free_(struct rte_mbuf *packet)
{
    rte_pktmbuf_free(packet);
}

struct rte_mbuf *rte_pktmbuf_alloc_(struct rte_mempool *mp)
{
    return rte_pktmbuf_alloc(mp);
}

uint16_t rte_eth_tx_burst_(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **tx_pkts, uint16_t nb_pkts)
{
    return rte_eth_tx_burst(port_id, queue_id, tx_pkts, nb_pkts);
}

uint16_t rte_eth_rx_burst_(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **rx_pkts, const uint16_t nb_pkts)
{
    return rte_eth_rx_burst(port_id, queue_id, rx_pkts, nb_pkts);
}

uint16_t rte_mbuf_refcnt_read_(const struct rte_mbuf *m)
{
    return rte_mbuf_refcnt_read(m);
}

uint16_t rte_mbuf_refcnt_update_(struct rte_mbuf *m, int16_t value)
{
    return rte_mbuf_refcnt_update(m, value);
}

char *rte_pktmbuf_adj_(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_adj(m, len);
}

int rte_pktmbuf_trim_(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_trim(m, len);
}

uint16_t rte_pktmbuf_headroom_(const struct rte_mbuf *m)
{
    return rte_pktmbuf_headroom(m);
}

uint16_t rte_pktmbuf_tailroom_(const struct rte_mbuf *m)
{
    return rte_pktmbuf_tailroom(m);
}

int rte_errno_()
{
    return rte_errno;
}

int rte_pktmbuf_chain_(struct rte_mbuf *head, struct rte_mbuf *tail)
{
    return rte_pktmbuf_chain(head, tail);
}

int rte_eth_rss_ip_()
{
    return RTE_ETH_RSS_IP;
}

int rte_eth_tx_offload_tcp_cksum_()
{
    return RTE_ETH_TX_OFFLOAD_TCP_CKSUM;
}

int rte_eth_rx_offload_tcp_cksum_()
{
    return RTE_ETH_RX_OFFLOAD_TCP_CKSUM;
}

int rte_eth_tx_offload_udp_cksum_()
{
    return RTE_ETH_TX_OFFLOAD_UDP_CKSUM;
}

int rte_eth_rx_offload_udp_cksum_()
{
    return RTE_ETH_RX_OFFLOAD_TCP_CKSUM;
}

int rte_eth_tx_offload_multi_segs_()
{
    return RTE_ETH_TX_OFFLOAD_MULTI_SEGS;
}

char *rte_pktmbuf_prepend_(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_prepend(m, len);
}

struct rte_mbuf *rte_mbuf_from_indirect_(struct rte_mbuf *mi)
{
    return rte_mbuf_from_indirect(mi);
}

void rte_pktmbuf_detach_(struct rte_mbuf *m)
{
    rte_pktmbuf_detach(m);
}

uint16_t rte_ring_sp_enqueue_burst_(struct rte_ring *r, struct rte_mbuf **tx_pkts, uint16_t n)
{
	return rte_ring_sp_enqueue_burst_elem(r, tx_pkts, sizeof(void *), n, NULL);
}

uint16_t rte_ring_sc_dequeue_burst_(struct rte_ring *r, struct rte_mbuf **rx_pkts, uint16_t n)
{
	return rte_ring_sc_dequeue_burst_elem(r, rx_pkts, sizeof(void *), n, NULL);
}

uint32_t parse_ipv4_ptype_(struct rte_mbuf *mbuf)
{
    uint32_t l3_ptype = mbuf->packet_type & RTE_PTYPE_L3_MASK;
    if (likely(l3_ptype > 0)) {
        if ((l3_ptype == RTE_PTYPE_L3_IPV4) || (l3_ptype == RTE_PTYPE_L3_IPV4_EXT)) {
            return mbuf->packet_type & RTE_PTYPE_L4_MASK;
        }
        return 0;
    } else {
        struct rte_ether_hdr *eth_hdr = rte_pktmbuf_mtod(mbuf, struct rte_ether_hdr *);
        if (eth_hdr->ether_type == rte_cpu_to_be_16(RTE_ETHER_TYPE_IPV4)) {
            struct rte_ipv4_hdr *ipv4_hdr = rte_pktmbuf_mtod_offset(mbuf, struct rte_ipv4_hdr *, sizeof(struct rte_ether_hdr));
            return ipv4_hdr->next_proto_id;
        }
        return 0;
    }
}