package de.sve.backend.model.news;

import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Subscription {

	public static Subscription create(String email, NewsType type) {
		return create(email, new HashSet<>(Arrays.asList(type)));
	}

	public static Subscription create(String email, Set<NewsType> types) {
		return new AutoValue_Subscription(email, types);
	}

	public abstract String email();

	public abstract Set<NewsType> types();

	public Subscription add(Subscription subscription) {
		Set<NewsType> types = new HashSet<>(types());
		types.addAll(subscription.types());
		return create(email(), types);
	}

	public Subscription remove(Subscription subscription) {
		Set<NewsType> types = new HashSet<>(types());
		types.removeAll(subscription.types());
		return create(email(), types);
	}

}
