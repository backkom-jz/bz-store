export const money=(cents:number)=>new Intl.NumberFormat('zh-CN',{style:'currency',currency:'CNY',minimumFractionDigits:cents%100===0?0:2}).format(cents/100);
export const dateTime=(value:string)=>new Intl.DateTimeFormat('zh-CN',{dateStyle:'medium',timeStyle:'short',timeZone:'Asia/Shanghai'}).format(new Date(value));
export const statusLabel:Record<string,string>={pending_payment:'待付款',payment_review:'付款审核中',paid:'已付款',fulfilling:'交付中',completed:'已完成',cancelled:'已取消',refunded:'已退款',pending:'待审核',approved:'已通过',rejected:'已驳回',draft:'草稿',published:'已发布',archived:'已下架'};
export function parseCents(value:string):number{
 if(!/^(0|[1-9]\d{0,6})(\.\d{1,2})?$/.test(value))throw new Error('金额必须为非负数，最多两位小数');
 const [whole,fraction='']=value.split('.');const cents=Number(whole)*100+Number(fraction.padEnd(2,'0'));
 if(!Number.isSafeInteger(cents)||cents>100_000_000)throw new Error('金额不能超过 1,000,000 元');return cents;
}
