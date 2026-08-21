//! Personal rewards bought with points earned from completed habits and
//! todos. Point values are fixed internal rules (design 11a): users never
//! see or edit the mapping. Earned points are derived from habit/todo data
//! on the fly; only what's been spent is stored.

use crate::persist;
use crate::store::Data;
use crate::todos::TodoData;
use serde::{Deserialize, Serialize};

const KEY: &str = "rewards/v1";

/// Points per completed habit day — same as an easy todo.
pub const HABIT_POINTS: u64 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reward {
    pub id: u64,
    pub name: String,
    pub cost: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewardData {
    next_id: u64,
    pub rewards: Vec<Reward>,
    /// Lifetime points spent on redemptions.
    spent: u64,
}

/// Total points ever earned. Derived, not stored: unticking a habit day or
/// todo un-earns its points automatically. Deleting a done habit/todo also
/// retroactively drops its points — the balance just saturates at zero.
pub fn earned(habits: &Data, todos: &TodoData) -> u64 {
    let habit_points: u64 = habits
        .habits
        .iter()
        .map(|habit| habit.days.len() as u64 * HABIT_POINTS)
        .sum();
    let todo_points: u64 = todos
        .todos
        .iter()
        .filter(|todo| todo.done)
        .map(|todo| todo.difficulty.points())
        .sum();
    habit_points + todo_points
}

impl RewardData {
    pub fn load() -> Self {
        persist::get(KEY).unwrap_or_default()
    }
    pub fn save(&self) {
        persist::set(KEY, self);
    }

    /// Spendable points given the earned total.
    pub fn balance(&self, earned: u64) -> u64 {
        earned.saturating_sub(self.spent)
    }

    pub fn add(&mut self, name: &str, cost: u32) {
        let name = name.trim();
        if name.is_empty() || cost == 0 {
            return;
        }
        self.rewards.push(Reward {
            id: self.next_id,
            name: name.to_string(),
            cost,
        });
        self.next_id += 1;
    }

    /// Replace a reward's name and cost. Empty names and zero costs are
    /// rejected, like `add`.
    pub fn update(&mut self, id: u64, name: &str, cost: u32) {
        let name = name.trim();
        if name.is_empty() || cost == 0 {
            return;
        }
        if let Some(reward) = self.rewards.iter_mut().find(|reward| reward.id == id) {
            reward.name = name.to_string();
            reward.cost = cost;
        }
    }

    /// Remove a reward. Past redemptions stay spent.
    pub fn delete(&mut self, id: u64) {
        self.rewards.retain(|reward| reward.id != id);
    }

    /// Spend a reward's cost. A no-op when the balance can't cover it, so
    /// a stale click on a just-unaffordable button can't go negative.
    pub fn redeem(&mut self, id: u64, earned: u64) {
        if let Some(reward) = self.rewards.iter().find(|reward| reward.id == id)
            && u64::from(reward.cost) <= self.balance(earned)
        {
            self.spent += u64::from(reward.cost);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todos::Difficulty;
    use chrono::NaiveDate;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, d).unwrap()
    }

    #[test]
    fn earned_counts_habit_days_and_done_todos_by_difficulty() {
        let mut habits = Data::default();
        habits.add("Stretch", Default::default(), 66);
        // Inserted directly: `toggle` only accepts days in the edit window
        // around the real today.
        habits.habits[0].days.extend([day(1), day(2)]);

        let mut todos = TodoData::default();
        todos.add("Easy thing", None, None, Difficulty::Easy);
        todos.add("Hard thing", None, None, Difficulty::Hard);
        todos.add("Not done", None, None, Difficulty::Hard);
        todos.toggle(0);
        todos.toggle(1);

        // 2 habit days × 10 + easy 10 + hard 40.
        assert_eq!(earned(&habits, &todos), 70);
    }

    #[test]
    fn redeem_spends_only_when_affordable() {
        let mut data = RewardData::default();
        data.add("Cake", 50);
        data.add("Movie night", 80);

        data.redeem(1, 70); // 80 > 70: no-op
        assert_eq!(data.balance(70), 70);

        data.redeem(0, 70); // 50 ≤ 70: spends
        assert_eq!(data.balance(70), 20);
        assert_eq!(data.rewards.len(), 2); // rewards are repeatable
    }

    #[test]
    fn balance_saturates_when_earned_points_are_retroactively_lost() {
        let mut data = RewardData::default();
        data.add("Cake", 50);
        data.redeem(0, 50);
        // The done todo backing those points was deleted afterwards.
        assert_eq!(data.balance(10), 0);
    }

    #[test]
    fn update_and_delete_edit_the_list_but_not_spent_points() {
        let mut data = RewardData::default();
        data.add("Cake", 50);
        data.redeem(0, 100);

        data.update(0, "Bigger cake", 80);
        assert_eq!(
            (data.rewards[0].name.as_str(), data.rewards[0].cost),
            ("Bigger cake", 80)
        );
        data.update(0, " ", 90); // rejected, like add
        assert_eq!(data.rewards[0].cost, 80);

        data.delete(0);
        assert!(data.rewards.is_empty());
        assert_eq!(data.balance(100), 50); // the redeemed 50 stays spent
    }

    #[test]
    fn empty_names_and_zero_costs_are_rejected() {
        let mut data = RewardData::default();
        data.add("  ", 10);
        data.add("Free lunch", 0);
        assert!(data.rewards.is_empty());
    }
}
