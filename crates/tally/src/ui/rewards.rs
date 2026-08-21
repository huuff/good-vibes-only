//! The reward shop (design 11a) and its create sheet (12a): spend points
//! earned from completed habits and todos.

use dioxus::prelude::*;

use super::Overlays;
use crate::clock;
use crate::i18n::fill;
use crate::preferences::Language;
use crate::rewards::{self, RewardData};
use crate::store::Data;
use crate::todos::TodoData;

pub fn rewards(
    mut data: Signal<RewardData>,
    habit_data: Signal<Data>,
    todo_data: Signal<TodoData>,
    overlays: Overlays,
    lang: Language,
) -> Element {
    let t = lang.strings();
    let today = clock::today();
    let earned = rewards::earned(&habit_data(), &todo_data());
    let balance = data().balance(earned);
    let snapshot = data();
    rsx! {
        section { class: "todos-screen",
            header { class: "todos-head",
                span { class: "head-date",
                    {today.format_localized("%a %-d %b %Y", lang.locale()).to_string()}
                }
                div { class: "rewards-title-row",
                    h1 { class: "title", {t.rewards_title} }
                    div { class: "balance",
                        strong { "{balance}" }
                        span { {t.points_label} }
                    }
                }
            }
            div { class: "todo-list",
                if snapshot.rewards.is_empty() {
                    div { class: "empty todo-empty",
                        strong { {t.empty_rewards} }
                        span { {t.empty_rewards_hint} }
                    }
                } else {
                    h2 { class: "todo-group-label", {t.your_rewards} }
                    for reward in snapshot.rewards {
                        {
                            let affordable = u64::from(reward.cost) <= balance;
                            let label = if affordable {
                                fill(t.redeem_reward, &[&reward.name])
                            } else {
                                fill(t.redeem_insufficient, &[&reward.name])
                            };
                            let edit_title = fill(t.edit_reward, &[&reward.name]);
                            let id = reward.id;
                            let row_reward = reward.clone();
                            let mut row_overlays = overlays;
                            let mut copy_overlays = overlays;
                            let copy_reward = reward.clone();
                            rsx! {
                                div {
                                    key: "{id}",
                                    class: "reward-row",
                                    onclick: move |_| row_overlays.open_edit_reward(&row_reward),
                                    button {
                                        class: "todo-copy",
                                        title: edit_title,
                                        onclick: move |event| {
                                            event.stop_propagation();
                                            copy_overlays.open_edit_reward(&copy_reward);
                                        },
                                        strong { class: "todo-name", {reward.name.clone()} }
                                        span { class: "todo-meta", {fill(t.cost_points, &[&reward.cost])} }
                                    }
                                    button {
                                        class: "redeem",
                                        disabled: !affordable,
                                        aria_label: label.clone(),
                                        title: label,
                                        onclick: move |event| {
                                            event.stop_propagation();
                                            let earned = rewards::earned(&habit_data(), &todo_data());
                                            data.write().redeem(id, earned);
                                            data().save();
                                        },
                                        {t.redeem}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The reward form sheet: creates when opened from the FAB, edits (same
/// form, prefilled, plus delete) when opened from a row.
pub fn add_sheet(mut data: Signal<RewardData>, mut overlays: Overlays, lang: Language) -> Element {
    let editing = (overlays.reward_edit)();
    if !(overlays.adding_reward)() && editing.is_none() {
        return rsx! {};
    }
    let t = lang.strings();
    let cost = (overlays.reward_cost)().parse::<u32>().unwrap_or(0);
    let valid = !(overlays.reward_name)().trim().is_empty() && cost > 0;
    rsx! {
        div { class: "overlay", role: "presentation", onclick: move |_| overlays.dismiss(),
            section { class: "sheet", role: "dialog", aria_modal: "true", aria_labelledby: "new-reward-title", onclick: move |event| event.stop_propagation(),
                div { class: "sheet-label",
                    if editing.is_some() { {t.edit_reward_label} } else { {t.new_reward_label} }
                }
                h2 { id: "new-reward-title", class: "sheet-name", {t.what_reward} }
                form { class: "form todo-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if valid {
                            match editing {
                                Some(id) => data.write().update(id, &(overlays.reward_name)(), cost),
                                None => data.write().add(&(overlays.reward_name)(), cost),
                            }
                            data().save();
                            overlays.dismiss();
                        }
                    },
                    label { class: "field-label", r#for: "reward-name", {t.reward_field} }
                    input { id: "reward-name", class: "input", autofocus: true, value: "{overlays.reward_name}", placeholder: t.reward_placeholder, oninput: move |event| overlays.reward_name.set(event.value()) }
                    label { class: "field-label", r#for: "reward-cost", {t.cost_field} }
                    div { class: "input cost-input",
                        input { id: "reward-cost", r#type: "number", min: "1", inputmode: "numeric", value: "{overlays.reward_cost}", oninput: move |event| overlays.reward_cost.set(event.value()) }
                        span { {t.points_label} }
                    }
                    p { class: "todo-form-hint", {t.reward_hint} }
                    button { class: "btn", r#type: "submit", disabled: !valid,
                        if editing.is_some() { {t.save} } else { {t.create_reward} }
                    }
                }
                if let Some(id) = editing {
                    div { class: "sheet-del",
                        if (overlays.confirm)() {
                            button {
                                class: "btn-quiet danger",
                                title: t.really_delete,
                                onclick: move |_| {
                                    overlays.dismiss();
                                    data.write().delete(id);
                                    data().save();
                                },
                                {t.sure}
                            }
                        } else {
                            button {
                                class: "btn-quiet",
                                title: t.delete_reward_title,
                                onclick: move |_| overlays.confirm.set(true),
                                {t.delete_reward}
                            }
                        }
                    }
                }
            }
        }
    }
}
